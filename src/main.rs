use std::collections::VecDeque;
use std::io::{self, Write};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossterm::{cursor, execute, terminal};
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use rustfft::{num_complex::Complex, FftPlanner};

const RATE: u32 = 48_000;
const WINDOW: usize = 2048;
const READ_FRAMES: usize = 1024;

/// Находит монитор дефолтного синка (то, что сейчас играет).
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

    // монитор дефолтного синка в приоритете
    if let Ok(o) = Command::new("pactl").arg("get-default-sink").output() {
        let sink = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !sink.is_empty() {
            let cand = format!("{sink}.monitor");
            if monitors.iter().any(|m| *m == cand) {
                return Some(cand);
            }
        }
    }
    monitors.into_iter().next()
}

fn capture_loop(buf: Arc<Mutex<VecDeque<f32>>>, monitor: Option<String>, running: Arc<AtomicBool>) {
    let spec = Spec {
        format: Format::S16le,
        channels: 1,
        rate: RATE,
    };
    let dev = monitor.as_deref();
    let simple = match Simple::new(
        None,
        "xvisualizer",
        Direction::Record,
        dev,
        "capture",
        &spec,
        None,
        None,
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pulse record error: {e:?}");
            running.store(false, Ordering::SeqCst);
            return;
        }
    };

    let mut raw = vec![0u8; READ_FRAMES * 2];
    while running.load(Ordering::SeqCst) {
        if simple.read(&mut raw).is_err() {
            thread::sleep(Duration::from_millis(50));
            continue;
        }
        let mut q = buf.lock().unwrap();
        for chunk in raw.chunks_exact(2) {
            let s = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
            q.push_back(s);
        }
        while q.len() > RATE as usize {
            q.pop_front();
        }
    }
}

fn print_help() {
    println!("xvisualizer — ASCII-визуалайзер системного аудио");
    println!();
    println!("Использование: xvisualizer [ОПЦИИ]");
    println!();
    println!("Опции:");
    println!("  -f, --fps N    скорость обновления столбиков, кадров/сек (1–240, по умолчанию 30)");
    println!("  -h, --help     показать эту справку");
}

fn parse_args() -> u32 {
    let mut fps = 30u32;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-f" | "--fps" => {
                let v = it.next().unwrap_or_else(|| {
                    eprintln!("ошибка: {a} требует значение (1–240)");
                    std::process::exit(2);
                });
                fps = v.parse().unwrap_or_else(|_| {
                    eprintln!("ошибка: fps должно быть числом, получено '{v}'");
                    std::process::exit(2);
                });
                if !(1..=240).contains(&fps) {
                    eprintln!("ошибка: fps должен быть 1–240, получено {fps}");
                    std::process::exit(2);
                }
            }
            _ => {
                eprintln!("ошибка: неизвестный флаг '{a}'. См. xvisualizer --help");
                std::process::exit(2);
            }
        }
    }
    fps
}

fn main() -> io::Result<()> {
    let fps = parse_args();
    let frame_time = Duration::from_secs_f64(1.0 / fps as f64);
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let _ = ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    });

    let monitor = detect_monitor();

    let buf: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
    {
        let b = buf.clone();
        let m = monitor.clone();
        let run = running.clone();
        thread::spawn(move || capture_loop(b, m, run));
    }

    // ждём первые данные (или тишину — нули тоже данные)
    thread::sleep(Duration::from_millis(400));

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WINDOW);
    let mut spectrum = vec![Complex::new(0.0, 0.0); WINDOW];

    // окно Ханна
    let hann: Vec<f32> = (0..WINDOW)
        .map(|n| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / WINDOW as f32).cos()))
        .collect();

    // логарифмическая сетка бинов: 30 Гц .. 16 кГц
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
    let mut peak = 1e-3f32; // адаптивное усиление

    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    execute!(stdout, terminal::Clear(terminal::ClearType::All))?;
    execute!(stdout, cursor::Hide)?;

    while running.load(Ordering::SeqCst) {
        // --- взять окно сэмплов ---
        let samples: Vec<f32> = {
            let q = buf.lock().unwrap();
            let n = q.len().min(WINDOW);
            let skip = q.len() - n;
            q.iter().skip(skip).copied().collect()
        };
        for (i, s) in spectrum.iter_mut().enumerate() {
            let v = samples.get(i).copied().unwrap_or(0.0) * hann[i];
            *s = Complex::new(v, 0.0);
        }
        fft.process(&mut spectrum);

        // --- магнитуды ---
        let mut mags = vec![0.0f32; WINDOW / 2];
        let mut frame_max = 1e-6f32;
        for (i, m) in mags.iter_mut().enumerate() {
            let c = spectrum[i + 1];
            let v = (c.re * c.re + c.im * c.im).sqrt() / WINDOW as f32;
            *m = v;
            if v > frame_max {
                frame_max = v;
            }
        }
        // автоволюм: быстрая атака, медленный спад
        peak = frame_max.max(peak * 0.995).max(1e-4);

        // --- размер/центровка ---
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let cols = cols as usize;
        let rows = rows as usize;
        let footer_h = 2usize;
        let area_h = rows.saturating_sub(footer_h).max(5);
        let n_bars = ((cols.saturating_sub(4)) / 2).clamp(8, max_bar_count);

        // --- бары ---
        for i in 0..n_bars {
            let lo = edges[i * max_bar_count / n_bars].min(WINDOW / 2 - 1);
            let hi = edges[(i + 1) * max_bar_count / n_bars].max(lo + 1).min(WINDOW / 2);
            let m = mags[lo..hi].iter().copied().fold(0.0f32, f32::max);
            let norm = (m / peak).clamp(0.0, 1.0);
            let target = norm.powf(0.6); // гамма — тихие частоты виднее
            bars[i] = target.max(bars[i] * 0.88);
        }

        let width = n_bars * 2 - 1;
        let pad_x = cols.saturating_sub(width) / 2;
        let tallest = bars
            .iter()
            .take(n_bars)
            .map(|v| (v * area_h as f32).round() as usize)
            .max()
            .unwrap_or(0)
            .min(area_h);
        let pad_top = (area_h - tallest) + (rows - area_h - footer_h) / 2;

        execute!(stdout, terminal::Clear(terminal::ClearType::All))?;
        execute!(stdout, cursor::MoveTo(0, 0))?;
        for _ in 0..pad_top {
            writeln!(stdout)?;
        }
        let pad = " ".repeat(pad_x);
        let heights: Vec<usize> = bars
            .iter()
            .take(n_bars)
            .map(|v| ((v * area_h as f32).round() as usize).min(area_h))
            .collect();
        for row in (0..tallest).rev() {
            write!(stdout, "{pad}")?;
            for (i, h) in heights.iter().enumerate() {
                if *h > row {
                    write!(stdout, "█")?;
                } else {
                    write!(stdout, " ")?;
                }
                if i + 1 < n_bars {
                    write!(stdout, " ")?;
                }
            }
            writeln!(stdout)?;
        }
        // футер
        let src = monitor.as_deref().unwrap_or("mic/default");
        let short = if src.len() > 40 { &src[..40] } else { src };
        let foot = format!("{short}  |  {fps} FPS  |  Ctrl+C");
        let foot_x = cols.saturating_sub(foot.chars().count()) / 2;
        writeln!(stdout)?;
        writeln!(stdout, "{}{}", " ".repeat(foot_x), foot)?;
        stdout.flush()?;

        thread::sleep(frame_time);
    }

    execute!(stdout, cursor::Show)?;
    execute!(stdout, terminal::LeaveAlternateScreen)?;
    Ok(())
}
