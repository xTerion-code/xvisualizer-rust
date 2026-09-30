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

/// Возвращает монитор дефолтного синка (то, что сейчас играет).
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
            if monitors.contains(&cand) {
                return Some(cand);
            }
        }
    }
    monitors.into_iter().next()
}

fn capture_loop(buf: Arc<Mutex<VecDeque<f32>>>, monitor: Option<String>, running: Arc<AtomicBool>) {
    // Просим стерео и усредняем в моно: мониторы Pulse обычно стерео,
    // запись 1 канала давала бы только левый.
    let spec = Spec {
        format: Format::S16le,
        channels: 2,
        rate: RATE,
    };
    let dev = monitor.as_deref();
    // Низкая задержка: дефолтный fragsize у Pulse ~2с — именно он давал
    // опоздание на секунды. Просим куски по ~10мс и маленький буфер (~43мс).
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
        // Декодируем БЕЗ удержания мьютекса, затем один короткий lock.
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
        // Короткая очередь (~170мс): старое стирается сразу, нового залежалого нет.
        let excess = q.len().saturating_sub(8192);
        if excess > 0 {
            q.drain(..excess);
        }
    }
}

/// Гарант возврата терминала в нормальное состояние (включая panic).
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
    println!("xvisualizer — ASCII-визуалайзер системного аудио");
    println!();
    println!("Использование: xvisualizer [ОПЦИИ]");
    println!();
    println!("Опции:");
    println!("  -d, --device NAME Pulse-источник (по умолчанию авто-монитор дефолтного синка)");
    println!("      --no-color    без ANSI-цветов");
    println!("  -h, --help        показать эту справку");
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
                    eprintln!("ошибка: {a} требует имя устройства");
                    std::process::exit(2);
                });
                device = Some(v);
            }
            "--no-color" => no_color = true,
            _ => {
                eprintln!("ошибка: неизвестный флаг '{a}'. См. xvisualizer --help");
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
    // Фиксированный высокий рефреш для максимально плавной анимации,
    // без пользовательской системы FPS.
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

    // ждём первые данные (или тишину — нули тоже данные)
    thread::sleep(Duration::from_millis(120));

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WINDOW);
    let mut spectrum = vec![Complex::new(0.0, 0.0); WINDOW];
    let mut samples = vec![0.0f32; WINDOW];
    let mut mags = vec![0.0f32; WINDOW / 2];

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
    let mut caps = vec![0.0f32; max_bar_count]; // падающие пиковые метки
    let mut bar_h = vec![0.0f32; max_bar_count];
    let mut cap_rows = vec![0usize; max_bar_count];
    let mut peak = 1e-3f32; // адаптивное усиление

    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    execute!(stdout, cursor::Hide)?;
    let _term_guard = TerminalGuard;

    // Переиспользуемый буфер кадра: один write + flush за кадр.
    let mut frame = String::with_capacity(80 * 30);
    let mut prev = Instant::now();

    while running.load(Ordering::SeqCst) {
        let frame_start = Instant::now();
        // dt для плавного затухания (clamp от скачков при ресайзе/лагах).
        let dt = frame_start
            .duration_since(prev)
            .as_secs_f32()
            .clamp(0.001, 0.05);
        prev = frame_start;
        // Плавно, но с мгновенной реакцией: атака очень быстрая (тау ~12мс,
        // 1-2 кадра при 120Гц), спад мягкий (тау ~160мс). Усиление адаптируется
        // за ~0.4с, пики падают за ~0.8с вместо ~3с.
        let k_atk = 1.0 - (-dt / 0.012).exp();
        let k_rel = 1.0 - (-dt / 0.16).exp();
        let peak_keep = (-dt / 0.4).exp();
        let cap_fall = dt * 1.2;

        // --- взять окно сэмплов (короткий lock, без аллокаций) ---
        {
            let q = match buf.lock() {
                Ok(q) => q,
                Err(_) => break,
            };
            let n = q.len().min(WINDOW);
            let skip = q.len() - n;
            // Новые данные — в конце окна, недостающее начало — тишина.
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

        // --- магнитуды ---
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
        peak = frame_max.max(peak * peak_keep).max(1e-4);

        // --- размер ---
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let cols = cols as usize;
        let rows = rows as usize;
        let footer_h = 2usize;
        let area_h = rows.saturating_sub(footer_h).max(5);
        let n_bars = ((cols.saturating_sub(4)) / 2).clamp(8, max_bar_count);

        // --- бары + пики (инерционные, без ступенек) ---
        for i in 0..n_bars {
            let lo = edges[i * max_bar_count / n_bars].min(WINDOW / 2 - 1);
            let hi = edges[(i + 1) * max_bar_count / n_bars]
                .max(lo + 1)
                .min(WINDOW / 2);
            let m = mags[lo..hi].iter().copied().fold(0.0f32, f32::max);
            let norm = (m / peak).clamp(0.0, 1.0);
            let target = norm.powf(0.6); // гамма — тихие частоты виднее
            let b = bars[i];
            // Плавно тянемся к цели: вверх быстро, вниз медленно.
            if target > b {
                bars[i] = b + (target - b) * k_atk;
            } else {
                bars[i] = b + (target - b) * k_rel;
            }
            // Пиковая метка: мгновенно вверх, линейно медленно вниз.
            if target >= caps[i] {
                caps[i] = target;
            } else {
                caps[i] = (caps[i] - cap_fall).max(target).max(0.0);
            }
            bar_h[i] = (bars[i] * area_h as f32).clamp(0.0, area_h as f32);
            cap_rows[i] = ((caps[i] * area_h as f32).round() as usize).min(area_h);
        }

        // --- рендер в буфер фиксированной высоты (без Clear → без мигания) ---
        let width = n_bars * 2 - 1;
        let pad_x = cols.saturating_sub(width) / 2;
        frame.clear();
        // Верхняя строка области; курсор ставим один раз.
        use std::fmt::Write as _;
        let _ = write!(frame, "\x1b[H");
        let pad: String = " ".repeat(pad_x);
        let use_color = !args.no_color;
        for row in (0..area_h).rev() {
            frame.push_str(&pad);
            // Цвет строки по высоте: зелёный → жёлтый → красный.
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
                // Дробная высота: целая часть — полные блоки,
                // дробная — 1/8-блоки для субклеточной плавности.
                let fh = bar_h[i];
                let full = fh.floor() as usize;
                let frac = fh - full as f32;
                if row < full {
                    frame.push('█');
                } else if row == full {
                    let idx = ((frac * 8.0).round() as usize).min(8);
                    // 1/8-блоки снизу вверх
                    const PARTS: [char; 9] =
                        [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
                    let ch = PARTS[idx];
                    if ch == ' ' {
                        // пусто — может быть пиковая метка
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
                    // пиковая метка ровно на одну клетку выше/на вершине
                    if use_color {
                        frame.push_str("\x1b[0m\x1b[36m─\x1b[0m");
                        // вернуть цвет строки для следующих клеток
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
            // Затереть остатки прошлой длинной строки при сужении.
            frame.push_str("\x1b[K\n");
        }
        // футер: ровно footer_h строк для фиксированной геометрии
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

        // Фиксированный высокий рефреш: спим остаток до ~120 Гц.
        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_TIME {
            thread::sleep(FRAME_TIME - elapsed);
        }
    }

    Ok(())
}
