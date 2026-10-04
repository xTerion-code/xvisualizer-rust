mod args;
mod audio;
mod choice_menu;
mod color;
mod color_menu;
mod dsp;
mod frame;
mod gain;
mod help;
mod input;
mod layout_menu;
mod solo;
mod source;
mod source_menu;
mod symmetry;
mod terminal;
mod theme;
mod theme_menu;
mod ui;
mod visualizer;

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::solo::SoloSession;
use crate::source::CaptureTarget;
use crate::ui::{UiEvent, UiState};

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
    let mut gain = gain::Gain::new();
    let mut ui = UiState::new(parsed.theme.unwrap_or(theme::Theme::Classic));

    // None = system mix; dropping the session moves the stream back
    // and unloads the helper modules.
    let mut solo: Option<SoloSession> = None;
    let mut notice: Option<(String, Instant)> = None;

    let mut stdout = std::io::stdout();
    let _guard = terminal::enter(&mut stdout)?;
    // Panics would otherwise leave raw mode / alternate screen on.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        terminal::restore();
        default_hook(info);
    }));

    // Single write + flush per frame to avoid flicker.
    let mut frame = String::with_capacity(80 * 30);
    let mut prev = Instant::now();

    while running.load(Ordering::SeqCst) {
        for event in input::poll_keys(&mut ui, &running) {
            match event {
                UiEvent::SelectSystem => {
                    drop(solo.take());
                    capture.switch(system_monitor.clone(), &running);
                    ui.source.target = CaptureTarget::System;
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
                            ui.source.target = CaptureTarget::System;
                            notice = Some((
                                format!("Source switch failed, back to system: {e}"),
                                Instant::now(),
                            ));
                        }
                    }
                }
                UiEvent::GainUp => {
                    gain.up();
                    notice = Some((format!("Sensitivity {:.2}x", gain.value()), Instant::now()));
                }
                UiEvent::GainDown => {
                    gain.down();
                    notice = Some((format!("Sensitivity {:.2}x", gain.value()), Instant::now()));
                }
                UiEvent::GainReset => {
                    gain.reset();
                    notice = Some(("Sensitivity 1.00x".to_string(), Instant::now()));
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
        // Keep capacity ahead of very wide terminals to avoid reallocs.
        let want = cols.saturating_mul(rows).saturating_mul(8).max(80 * 30);
        if frame.capacity() < want {
            frame.reserve(want - frame.capacity());
        }
        // Paused freezes bars and peaks; dt keeps updating so resume is smooth.
        // Symmetric layout mirrors half the bands around the center,
        // left-to-right uses the full width directly.
        if !ui.paused {
            let display_fit = ui.theme.current.fit_bar_count(cols, dsp::MAX_BARS);
            let bar_count = match ui.layout.current {
                symmetry::LayoutMode::Symmetric => symmetry::unique_count(display_fit),
                symmetry::LayoutMode::LeftToRight => display_fit.max(1),
            };
            if !analyzer.update(capture.queue(), bar_count, dt, gain.value()) {
                break;
            }
        }

        let use_color = !parsed.no_color;
        let active_notice = match &notice {
            Some((msg, at)) if at.elapsed() < NOTICE_TIME => Some(msg.as_str()),
            _ => {
                notice = None;
                None
            }
        };
        if ui.source.open {
            source_menu::render(&mut frame, cols, rows, &ui.source, ui.setup, use_color);
        } else if ui.theme.open {
            theme_menu::render(
                &mut frame,
                cols,
                rows,
                ui.theme.selected,
                ui.theme.current,
                ui.setup,
                use_color,
            );
        } else if ui.color.open {
            color_menu::render(
                &mut frame,
                cols,
                rows,
                ui.color.selected,
                ui.color.current,
                use_color,
            );
        } else if ui.layout.open {
            layout_menu::render(
                &mut frame,
                cols,
                rows,
                ui.layout.selected,
                ui.layout.current,
                use_color,
            );
        } else if ui.in_help {
            help::render_help(&mut frame, cols, rows, use_color);
        } else {
            let (bar_width, gap_width) = ui.theme.current.dims();
            let system_label = system_monitor.as_deref().unwrap_or("mic/default");
            let source_label = ui.source.target.label(system_label);
            visualizer::render_visualizer(
                &mut frame,
                &visualizer::Visualizer {
                    bars: analyzer.bars(),
                    peaks: analyzer.peaks(),
                    cols,
                    rows,
                    bar_width,
                    gap_width,
                    use_color,
                    source: &source_label,
                    theme_name: ui.theme.current.name(),
                    notice: active_notice,
                    color_mode: ui.color.current,
                    layout: ui.layout.current,
                    paused: ui.paused,
                    gain: gain.value(),
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
