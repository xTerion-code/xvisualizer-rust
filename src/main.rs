use std::collections::VecDeque;
use std::io::{self, Write};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{cursor, execute, terminal};
use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use rustfft::{num_complex::Complex, FftPlanner};

const RATE: u32 = 48_000;
const WINDOW: usize = 2048;
const READ_FRAMES: usize = 512;

/// Returns the monitor of the default sink (whatever is currently playing).
fn detect_monitor() -> Option<String> {
    let sinks = Command::new("pactl")
        .arg("list")
        .arg("short")
        .arg("sources")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let monitors: Vec<String> = sinks
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .filter(|n| n.contains("monitor"))
        .map(|s| s.to_string())
        .collect();

    if monitors.is_empty() {
        return None;
    }

    // Prefer the monitor of the default sink.
    if let Ok(o) = Command::new("pactl").arg("get-default-sink").output() {
        let sink = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !sink.is_empty() {
            let cand = format!("{sink}.monitor");
            if monitors.contains(&cand) {
                return Some(cand);
            }
        }
    }
    monitors.into_iter().next()
}

fn capture_loop(buf: Arc<Mutex<VecDeque<f32>>>, monitor: Option<String>, running: Arc<AtomicBool>) {
    // Request stereo and average down to mono: Pulse monitors are usually stereo,
    // recording a single channel would capture the left channel only.
    let spec = Spec {
        format: Format::S16le,
        channels: 2,
        rate: RATE,
    };
    let dev = monitor.as_deref();
    // Low latency: Pulse's default fragsize is ~2s — that was the source of the
    // multi-second delay. Request ~10ms chunks and a small buffer (~43ms).
    let attr = BufferAttr {
        maxlength: 8192,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: (READ_FRAMES * 4) as u32,
    };
    let simple = match Simple::new(
        None,
        "xvisualizer",
        Direction::Record,
        dev,
        "capture",
        &spec,
        None,
        Some(&attr),
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pulse record error: {e:?}");
            running.store(false, Ordering::SeqCst);
            return;
        }
    };

    let mut raw = vec![0u8; READ_FRAMES * 4];
    let mut mono = vec![0.0f32; READ_FRAMES];
    while running.load(Ordering::SeqCst) {
        if simple.read(&mut raw).is_err() {
            thread::sleep(Duration::from_millis(50));
            continue;
        }
        // Decode WITHOUT holding the mutex, then take one short lock.
        for (i, frame) in raw.as_chunks::<4>().0.iter().enumerate() {
            let l = i16::from_le_bytes([frame[0], frame[1]]) as f32;
            let r = i16::from_le_bytes([frame[2], frame[3]]) as f32;
            mono[i] = (l + r) / 2.0 / 32768.0;
        }
        let mut q = match buf.lock() {
            Ok(q) => q,
            Err(_) => return,
        };
        q.extend(mono.iter().copied());
        // Short queue (~170ms): old samples are discarded immediately,
        // nothing stale piles up.
        let excess = q.len().saturating_sub(8192);
        if excess > 0 {
            q.drain(..excess);
        }
    }
}

/// Guarantees the terminal is restored to a normal state (including on panic).
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = execute!(out, cursor::Show);
        let _ = execute!(out, terminal::LeaveAlternateScreen);
    }
}

struct Args {
    device: Option<String>,
    no_color: bool,
}

fn print_help() {
    println!("xvisualizer — ASCII visualizer for system audio");
    println!();
    println!("Usage: xvisualizer [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -d, --device NAME Pulse source (default: auto-detected monitor of the default sink)");
    println!("      --no-color    disable ANSI colors");
    println!("  -h, --help        show this help");
}

fn parse_args() -> Args {
    let mut device: Option<String> = None;
    let mut no_color = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-d" | "--device" => {
                let v = it.next().unwrap_or_else(|| {
                    eprintln!("error: {a} requires a device name");
                    std::process::exit(2);
                });
                device = Some(v);
            }
            "--no-color" => no_color = true,
            _ => {
                eprintln!("error: unknown flag '{a}'. See xvisualizer --help");
                std::process::exit(2);
            }
        }
    }
    Args { device, no_color }
}

fn truncate_chars(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        return s;
    }
    let idx = s
        .char_indices()
        .nth(max)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    &s[..idx]
}

fn main() -> io::Result<()> {
    let args = parse_args();
    // Fixed high refresh rate for maximally smooth animation,
    // no user-facing FPS system.
    const FRAME_TIME: Duration = Duration::from_nanos(1_000_000_000 / 120);
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let _ = ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    });

    let monitor = args.device.clone().or_else(detect_monitor);

    let buf: Arc<Mutex<VecDeque<f32>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
    {
        let b = buf.clone();
        let m = monitor.clone();
        let run = running.clone();
        thread::spawn(move || capture_loop(b, m, run));
    }

    // Wait for the first data (silence — zeros — counts as data too).
    thread::sleep(Duration::from_millis(120));

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WINDOW);
    let mut spectrum = vec![Complex::new(0.0, 0.0); WINDOW];
    let mut samples = vec![0.0f32; WINDOW];
    let mut mags = vec![0.0f32; WINDOW / 2];

    // Hann window.
    let hann: Vec<f32> = (0..WINDOW)
        .map(|n| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / WINDOW as f32).cos()))
        .collect();

    // Logarithmic bin grid: 30 Hz .. 16 kHz.
    let bin_hz = RATE as f32 / WINDOW as f32;
    let f_min = 30.0_f32;
    let f_max = 16_000.0_f32;
    let max_bar_count = 64;
    let mut edges = Vec::with_capacity(max_bar_count + 1);
    for i in 0..=max_bar_count {
        let f = f_min * (f_max / f_min).powf(i as f32 / max_bar_count as f32);
        edges.push((f / bin_hz) as usize);
    }

    let mut bars = vec![0.0f32; max_bar_count];
    let mut caps = vec![0.0f32; max_bar_count]; // falling peak markers
    let mut bar_h = vec![0.0f32; max_bar_count];
    let mut cap_rows = vec![0usize; max_bar_count];
    let mut peak = 1e-3f32; // adaptive gain

    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    execute!(stdout, cursor::Hide)?;
    let _term_guard = TerminalGuard;

    // Reused frame buffer: a single write + flush per frame.
    let mut frame = String::with_capacity(80 * 30);
    let mut prev = Instant::now();

    while running.load(Ordering::SeqCst) {
        let frame_start = Instant::now();
        // dt for smooth decay (clamped against resize/lag spikes).
        let dt = frame_start
            .duration_since(prev)
            .as_secs_f32()
            .clamp(0.001, 0.05);
        prev = frame_start;
        // Smooth yet instantly responsive: very fast attack (tau ~12ms,
        // 1-2 frames at 120Hz), soft release (tau ~160ms). The gain adapts
        // in ~0.4s, peaks fall in ~0.8s.
        let k_atk = 1.0 - (-dt / 0.012).exp();
        let k_rel = 1.0 - (-dt / 0.16).exp();
        let peak_keep = (-dt / 0.4).exp();
        let cap_fall = dt * 1.2;

        // --- grab the sample window (short lock, no allocations) ---
        {
            let q = match buf.lock() {
                Ok(q) => q,
                Err(_) => break,
            };
            let n = q.len().min(WINDOW);
            let skip = q.len() - n;
            // New data goes at the end of the window, the missing head is silence.
            let offset = WINDOW - n;
            samples[..offset].fill(0.0);
            for (dst, src) in samples[offset..].iter_mut().zip(q.iter().skip(skip)) {
                *dst = *src;
            }
        }
        for (s, (v, w)) in spectrum.iter_mut().zip(samples.iter().zip(hann.iter())) {
            *s = Complex::new(v * w, 0.0);
        }
        fft.process(&mut spectrum);

        // --- magnitudes ---
        let mut frame_max = 1e-6f32;
        for (i, m) in mags.iter_mut().enumerate() {
            let c = spectrum[i + 1];
            let v = (c.re * c.re + c.im * c.im).sqrt() / WINDOW as f32;
            *m = v;
            if v > frame_max {
                frame_max = v;
            }
        }
        // Auto-volume: fast attack, slow release.
        peak = frame_max.max(peak * peak_keep).max(1e-4);

        // --- size ---
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let cols = cols as usize;
        let rows = rows as usize;
        let footer_h = 2usize;
        let area_h = rows.saturating_sub(footer_h).max(5);
        let n_bars = ((cols.saturating_sub(4)) / 2).clamp(8, max_bar_count);

        // --- bars + peaks (inertial, no stepping) ---
        for i in 0..n_bars {
            let lo = edges[i * max_bar_count / n_bars].min(WINDOW / 2 - 1);
            let hi = edges[(i + 1) * max_bar_count / n_bars]
                .max(lo + 1)
                .min(WINDOW / 2);
            let m = mags[lo..hi].iter().copied().fold(0.0f32, f32::max);
            let norm = (m / peak).clamp(0.0, 1.0);
            let target = norm.powf(0.6); // gamma — quiet frequencies stay visible
            let b = bars[i];
            // Ease towards the target: fast up, slow down.
            if target > b {
                bars[i] = b + (target - b) * k_atk;
            } else {
                bars[i] = b + (target - b) * k_rel;
            }
            // Peak marker: instantly up, linearly slowly down.
            if target >= caps[i] {
                caps[i] = target;
            } else {
                caps[i] = (caps[i] - cap_fall).max(target).max(0.0);
            }
            bar_h[i] = (bars[i] * area_h as f32).clamp(0.0, area_h as f32);
            cap_rows[i] = ((caps[i] * area_h as f32).round() as usize).min(area_h);
        }

        // --- render into a fixed-height buffer (no Clear → no flicker) ---
        let width = n_bars * 2 - 1;
        let pad_x = cols.saturating_sub(width) / 2;
        frame.clear();
        // Top row of the area; position the cursor once.
        use std::fmt::Write as _;
        let _ = write!(frame, "\x1b[H");
        let pad: String = " ".repeat(pad_x);
        let use_color = !args.no_color;
        for row in (0..area_h).rev() {
            frame.push_str(&pad);
            // Row color by height: green → yellow → red.
            if use_color {
                let ratio = row as f32 / area_h.max(1) as f32;
                if ratio >= 0.85 {
                    frame.push_str("\x1b[31m"); // red
                } else if ratio >= 0.6 {
                    frame.push_str("\x1b[33m"); // yellow
                } else {
                    frame.push_str("\x1b[32m"); // green
                }
            }
            for i in 0..n_bars {
                // Fractional height: the integer part is full blocks,
                // the fraction is 1/8-blocks for sub-cell smoothness.
                let fh = bar_h[i];
                let full = fh.floor() as usize;
                let frac = fh - full as f32;
                if row < full {
                    frame.push('█');
                } else if row == full {
                    let idx = ((frac * 8.0).round() as usize).min(8);
                    // 1/8-blocks, bottom to top.
                    const PARTS: [char; 9] =
                        [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
                    let ch = PARTS[idx];
                    if ch == ' ' {
                        // Empty — may hold a peak marker.
                        if cap_rows[i] == row + 1 && cap_rows[i] <= area_h {
                            if use_color {
                                frame.push_str("\x1b[0m\x1b[36m─\x1b[0m");
                                let ratio = row as f32 / area_h.max(1) as f32;
                                if ratio >= 0.85 {
                                    frame.push_str("\x1b[31m");
                                } else if ratio >= 0.6 {
                                    frame.push_str("\x1b[33m");
                                } else {
                                    frame.push_str("\x1b[32m");
                                }
                            } else {
                                frame.push('─');
                            }
                        } else {
                            frame.push(' ');
                        }
                    } else {
                        frame.push(ch);
                    }
                } else if cap_rows[i] > row && cap_rows[i] <= area_h && (cap_rows[i] == row + 1) {
                    // Peak marker exactly one cell above/at the top.
                    if use_color {
                        frame.push_str("\x1b[0m\x1b[36m─\x1b[0m");
                        // Restore the row color for the following cells.
                        let ratio = row as f32 / area_h.max(1) as f32;
                        if ratio >= 0.85 {
                            frame.push_str("\x1b[31m");
                        } else if ratio >= 0.6 {
                            frame.push_str("\x1b[33m");
                        } else {
                            frame.push_str("\x1b[32m");
                        }
                    } else {
                        frame.push('─');
                    }
                } else {
                    frame.push(' ');
                }
                if i + 1 < n_bars {
                    if use_color {
                        frame.push_str("\x1b[0m ");
                        let ratio = row as f32 / area_h.max(1) as f32;
                        if ratio >= 0.85 {
                            frame.push_str("\x1b[31m");
                        } else if ratio >= 0.6 {
                            frame.push_str("\x1b[33m");
                        } else {
                            frame.push_str("\x1b[32m");
                        }
                    } else {
                        frame.push(' ');
                    }
                }
            }
            if use_color {
                frame.push_str("\x1b[0m");
            }
            // Erase leftovers of a previously longer line when shrinking.
            frame.push_str("\x1b[K\n");
        }
        // Footer: exactly footer_h lines for a fixed geometry.
        let src = monitor.as_deref().unwrap_or("mic/default");
        let short = truncate_chars(src, 40);
        let foot = format!("{short}  |  Ctrl+C");
        let foot_w = foot.chars().count();
        let foot_x = cols.saturating_sub(foot_w) / 2;
        frame.push('\n');
        frame.push_str(&" ".repeat(foot_x));
        frame.push_str(&foot);
        frame.push_str("\x1b[K");

        stdout.write_all(frame.as_bytes())?;
        stdout.flush()?;

        // Fixed high refresh: sleep the remainder up to ~120 Hz.
        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_TIME {
            thread::sleep(FRAME_TIME - elapsed);
        }
    }

    Ok(())
}
