mod args;
mod audio;
mod dsp;
mod input;
mod render;
mod source;
mod terminal;
mod theme;

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::input::UiEvent;
use crate::source::{CaptureTarget, SoloSession};

const NOTICE_TIME: Duration = Duration::from_secs(4);

fn main() -> std::io::Result<()> {
    let parsed = args::parse_args();
    const FRAME_TIME: Duration = Duration::from_nanos(1_000_000_000 / 120);
    let running = Arc::new(AtomicBool::new(true));
    let stop = running.clone();
    let _ = ctrlc::set_handler(move || {
        stop.store(false, Ordering::SeqCst);
    });

    let system_monitor = parsed.device.clone().or_else(audio::detect_monitor);
    let mut capture = audio::CaptureSession::start(system_monitor.clone(), running.clone());

    thread::sleep(Duration::from_millis(120));

    let mut analyzer = dsp::Analyzer::new();
    let mut ui = input::UiState::new(parsed.theme.unwrap_or(theme::Theme::Classic));

    // None = system mix; dropping the session moves the stream back
    // and unloads the helper modules.
    let mut solo: Option<SoloSession> = None;
    let mut notice: Option<(String, Instant)> = None;

    let mut stdout = std::io::stdout();
    terminal::enter(&mut stdout)?;
    let _guard = terminal::TerminalGuard;

    // Single write + flush per frame to avoid flicker.
    let mut frame = String::with_capacity(80 * 30);
    let mut prev = Instant::now();

    while running.load(Ordering::SeqCst) {
        for event in input::poll_keys(&mut ui, &running) {
            match event {
                UiEvent::SelectSystem => {
                    drop(solo.take());
                    capture.switch(system_monitor.clone(), &running);
                    ui.target = CaptureTarget::System;
                    notice = Some(("Capturing system mix".to_string(), Instant::now()));
                }
                UiEvent::SelectStream(app) => {
                    drop(solo.take());
                    match SoloSession::start(&app) {
                        Ok(session) => {
                            let dev = Some(session.monitor_source());
                            capture.switch(dev, &running);
                            if session.loopback_active() {
                                notice = Some((
                                    format!("Capturing {} (still audible)", app.label()),
                                    Instant::now(),
                                ));
                            } else {
                                notice = Some((
                                    format!("Capturing {} (muted: no loopback)", app.label()),
                                    Instant::now(),
                                ));
                            }
                            solo = Some(session);
                        }
                        Err(e) => {
                            capture.switch(system_monitor.clone(), &running);
                            ui.target = CaptureTarget::System;
                            notice = Some((
                                format!("Source switch failed, back to system: {e}"),
                                Instant::now(),
                            ));
                        }
                    }
                }
            }
        }
        if !running.load(Ordering::SeqCst) {
            break;
        }
        let frame_start = Instant::now();
        // Clamp dt against resize/lag spikes so smoothing stays stable.
        let dt = frame_start
            .duration_since(prev)
            .as_secs_f32()
            .clamp(0.001, 0.05);
        prev = frame_start;

        let (cols, rows) = terminal::size();
        let bar_count = ui.current.fit_bar_count(cols, dsp::MAX_BARS);
        if !analyzer.update(capture.queue(), bar_count, dt) {
            break;
        }

        let use_color = !parsed.no_color;
        let active_notice = match &notice {
            Some((msg, at)) if at.elapsed() < NOTICE_TIME => Some(msg.as_str()),
            _ => {
                notice = None;
                None
            }
        };
        if ui.in_source_menu {
            render::render_source_menu(
                &mut frame,
                cols,
                rows,
                &ui.apps,
                ui.source_selected,
                &ui.target,
                use_color,
            );
        } else if ui.in_menu {
            render::render_menu(&mut frame, cols, rows, ui.selected, ui.current, use_color);
        } else {
            let (bar_width, gap_width) = ui.current.dims();
            let system_label = system_monitor.as_deref().unwrap_or("mic/default");
            let source_label = ui.target.label(system_label);
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
                    source: &source_label,
                    theme_name: ui.current.name(),
                    notice: active_notice,
                },
            );
        }

        stdout.write_all(frame.as_bytes())?;
        stdout.flush()?;

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_TIME {
            thread::sleep(FRAME_TIME - elapsed);
        }
    }

    drop(solo);

    Ok(())
}
