use std::fmt::Write as _;

use crate::color::{ColorMode, bar_color};
use crate::frame::truncate_chars;

const FOOTER_HEIGHT: usize = 2;

pub struct Visualizer<'a> {
    pub bars: &'a [f32],
    pub peaks: &'a [f32],
    pub cols: usize,
    pub rows: usize,
    pub bar_width: usize,
    pub gap_width: usize,
    pub use_color: bool,
    pub source: &'a str,
    pub theme_name: &'a str,
    pub notice: Option<&'a str>,
    pub color_mode: ColorMode,
    pub paused: bool,
    pub gain: f32,
    pub auto_gain: bool,
}

const PARTS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn render_visualizer(frame: &mut String, v: &Visualizer) {
    let n_bars = v.bars.len().min(crate::dsp::MAX_BARS);
    let area_h = v.rows.saturating_sub(FOOTER_HEIGHT).max(5);

    // Stack buffers: no per-frame heap allocation for heights.
    let mut bar_h = [0.0f32; crate::dsp::MAX_BARS];
    let mut cap_rows = [0usize; crate::dsp::MAX_BARS];
    for (i, b) in v.bars.iter().take(n_bars).enumerate() {
        bar_h[i] = (b * area_h as f32).clamp(0.0, area_h as f32);
    }
    for (i, c) in v.peaks.iter().take(n_bars).enumerate() {
        cap_rows[i] = ((c * area_h as f32).round() as usize).min(area_h);
    }

    let width = n_bars * v.bar_width + n_bars.saturating_sub(1) * v.gap_width;
    let pad_x = v.cols.saturating_sub(width) / 2;
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let gap_str: String = if v.gap_width > 0 {
        " ".repeat(v.gap_width)
    } else {
        String::new()
    };
    for row in (0..area_h).rev() {
        for _ in 0..pad_x {
            frame.push(' ');
        }
        // Per-bar colors need a reset between bars; Height mode is uniform
        // per row so a single prefix suffices, Frequency/Mono vary per bar.
        let uniform_row = matches!(v.color_mode, ColorMode::Height);
        if v.use_color && uniform_row {
            frame.push_str(bar_color(row, area_h, 0, n_bars, v.color_mode));
        }
        for i in 0..n_bars {
            if v.use_color && !uniform_row {
                frame.push_str(bar_color(row, area_h, i, n_bars, v.color_mode));
            }
            // Integer part is full blocks, the fraction is 1/8-blocks.
            let fh = bar_h[i];
            let full = fh.floor() as usize;
            let frac = fh - full as f32;
            let has_peak = cap_rows[i] == row + 1 && cap_rows[i] <= area_h && row >= full;
            if row < full {
                for _ in 0..v.bar_width {
                    frame.push('█');
                }
            } else if row == full {
                let idx = ((frac * 8.0).round() as usize).min(8);
                let ch = PARTS[idx];
                if ch == ' ' {
                    if has_peak {
                        push_peak(frame, v.bar_width, v.use_color);
                        if v.use_color {
                            frame.push_str(bar_color(row, area_h, i, n_bars, v.color_mode));
                        }
                    } else {
                        for _ in 0..v.bar_width {
                            frame.push(' ');
                        }
                    }
                } else {
                    for _ in 0..v.bar_width {
                        frame.push(ch);
                    }
                }
            } else if has_peak {
                push_peak(frame, v.bar_width, v.use_color);
                if v.use_color {
                    // Peak uses its own color; restore the bar color after it.
                    frame.push_str(bar_color(row, area_h, i, n_bars, v.color_mode));
                }
            } else {
                for _ in 0..v.bar_width {
                    frame.push(' ');
                }
            }
            if v.gap_width > 0 && i + 1 < n_bars {
                if v.use_color {
                    frame.push_str("\x1b[0m");
                    frame.push_str(&gap_str);
                    if uniform_row {
                        frame.push_str(bar_color(row, area_h, 0, n_bars, v.color_mode));
                    }
                } else {
                    frame.push_str(&gap_str);
                }
            }
            if v.use_color && !uniform_row {
                frame.push_str("\x1b[0m");
            }
        }
        if v.use_color {
            frame.push_str("\x1b[0m");
        }
        // Clear leftovers when shrinking to a narrower frame.
        frame.push_str("\x1b[K\r\n");
    }
    let gain_tag = if v.auto_gain { "AUTO" } else { "MAN" };
    let status = match v.notice {
        Some(msg) => truncate_chars(msg, 60).to_string(),
        None if v.paused => format!(
            "Paused - Space to resume  |  {:.1}x {}  |  {}",
            v.gain,
            gain_tag,
            v.color_mode.name()
        ),
        None => {
            let short = truncate_chars(v.source, 24);
            format!(
                "{short}  |  {}  |  {:.1}x {}  |  {}",
                v.theme_name,
                v.gain,
                gain_tag,
                v.color_mode.name()
            )
        }
    };
    let keys = "T-themes S-source Space-pause +/-gain G-reset A-auto C-color ?-help Q-quit";
    // Bar rows each end with `\r\n`, so the cursor is already on the first
    // footer line: write status directly, newline only before the keys line.
    // A leading `\r\n` before both lines would make the frame one line
    // taller than the screen and scroll on every frame (duplicated rows).
    for (idx, line) in [status.as_str(), keys].iter().enumerate() {
        let w = line.chars().count();
        let x = v.cols.saturating_sub(w) / 2;
        if idx == 1 {
            frame.push_str("\r\n");
        }
        frame.push_str(&" ".repeat(x));
        frame.push_str(line);
        frame.push_str("\x1b[K");
    }
}

fn push_peak(frame: &mut String, bar_width: usize, use_color: bool) {
    if use_color {
        frame.push_str("\x1b[0m\x1b[36m");
    }
    for _ in 0..bar_width {
        frame.push('─');
    }
    if use_color {
        frame.push_str("\x1b[0m");
    }
}
