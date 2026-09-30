//! xvisualizer - ASCII visualizer for system audio.
//!
//! Entry point and frame-loop orchestration. All real work lives in modules:
//! `args` (CLI), `audio` (capture), `dsp` (spectrum), `input` (keyboard),
//! `render` (frames), `terminal` (terminal mode), `theme` (bar styles).

mod args;
mod audio;
mod dsp;
mod input;
mod render;
mod terminal;
mod theme;

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> std::io::Result<()> {
    let parsed = args::parse_args();
    // Fixed high refresh rate for maximally smooth animation,
    // no user-facing FPS setting.
    const FRAME_TIME: Duration = Duration::from_nanos(1_000_000_000 / 120);
    let running = Arc::new(AtomicBool::new(true));
    let stop = running.clone();
    let _ = ctrlc::set_handler(move || {
        stop.store(false, Ordering::SeqCst);
    });

    let monitor = parsed.device.clone().or_else(audio::detect_monitor);
    let queue = audio::start_capture(monitor.clone(), running.clone());

    // Wait for the first data (silence - zeros - counts as data too).
    thread::sleep(Duration::from_millis(120));

    let mut analyzer = dsp::Analyzer::new();
    let mut ui = input::UiState::new(parsed.theme.unwrap_or(theme::Theme::Classic));

    let mut stdout = std::io::stdout();
    terminal::enter(&mut stdout)?;
    let _guard = terminal::TerminalGuard;

    // Reused frame buffer: a single write + flush per frame.
    let mut frame = String::with_capacity(80 * 30);
    let mut prev = Instant::now();

    while running.load(Ordering::SeqCst) {
        input::poll_keys(&mut ui, &running);
        if !running.load(Ordering::SeqCst) {
            break;
        }
        let frame_start = Instant::now();
        // dt for smooth decay (clamped against resize/lag spikes).
        let dt = frame_start
            .duration_since(prev)
            .as_secs_f32()
            .clamp(0.001, 0.05);
        prev = frame_start;

        let (cols, rows) = terminal::size();
        let bar_count = ui.current.fit_bar_count(cols, dsp::MAX_BARS);
        if !analyzer.update(&queue, bar_count, dt) {
            break;
        }

        let use_color = !parsed.no_color;
        if ui.in_menu {
            render::render_menu(&mut frame, cols, rows, ui.selected, ui.current, use_color);
        } else {
            let (bar_width, gap_width) = ui.current.dims();
            render::render_visualizer(
                &mut frame,
                &render::Visualizer {
                    bars: analyzer.bars(),
                    peaks: analyzer.peaks(),
                    cols,
                    rows,
                    bar_width,
                    gap_width,
                    use_color,
                    source: monitor.as_deref().unwrap_or("mic/default"),
                    theme_name: ui.current.name(),
                },
            );
        }

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
