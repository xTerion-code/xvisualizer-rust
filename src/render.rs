//! Frame rendering: theme menu and spectrum visualizer.
//!
//! Everything here only builds text into the reused frame buffer.
//! It performs no I/O and reads no input.
//!
//! Newlines are always `\r\n` because the terminal runs in raw mode,
//! where a bare `\n` does not return the carriage (staircase effect).

use std::fmt::Write as _;

use crate::theme::Theme;

/// Footer height in rows; the bar area is terminal height minus this.
const FOOTER_HEIGHT: usize = 2;

pub fn truncate_chars(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        return s;
    }
    let idx = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    &s[..idx]
}

/// Row color by height: green -> yellow -> red.
fn row_color(row: usize, area_h: usize) -> &'static str {
    let ratio = row as f32 / area_h.max(1) as f32;
    if ratio >= 0.85 {
        "\x1b[31m" // red
    } else if ratio >= 0.6 {
        "\x1b[33m" // yellow
    } else {
        "\x1b[32m" // green
    }
}

fn center_x(cols: usize, width: usize) -> usize {
    cols.saturating_sub(width) / 2
}

/// Draws the theme selection menu into `frame`.
///
/// Controls: arrows select, Enter applies.
pub fn render_menu(
    frame: &mut String,
    cols: usize,
    rows: usize,
    selected: usize,
    current: Theme,
    use_color: bool,
) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    // Vertical centering for the content below.
    let lines = 9usize;
    let mut top = rows.saturating_sub(lines) / 2;
    for _ in 0..top {
        frame.push_str("\x1b[K\r\n");
    }
    let title = "Select theme";
    let tx = center_x(cols, title.chars().count());
    frame.push_str(&" ".repeat(tx));
    if use_color {
        frame.push_str("\x1b[1;36m");
    }
    frame.push_str(title);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K\r\n");

    let hint_top = "Up/Down or Left/Right - select  |  Enter - apply";
    let hx = center_x(cols, hint_top.chars().count());
    frame.push_str(&" ".repeat(hx));
    if use_color {
        frame.push_str("\x1b[90m");
    }
    frame.push_str(hint_top);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K\r\n\r\n\x1b[K\r\n");

    for (idx, th) in Theme::all().iter().enumerate() {
        let is_sel = idx == selected;
        let is_cur = *th == current;
        // Preview: classic with gaps, solid joined.
        let preview = match th {
            Theme::Classic => "blocks with gaps",
            Theme::Solid => "joined blocks",
        };
        let marker = if is_sel { "> " } else { "  " };
        let cur_tag = if is_cur { "  (current)" } else { "" };
        let line = format!(
            "{marker}{} - {}  [{preview}]{cur_tag}",
            th.name(),
            th.desc()
        );
        let w = line.chars().count();
        let x = center_x(cols, w);
        frame.push_str(&" ".repeat(x));
        if is_sel {
            if use_color {
                frame.push_str("\x1b[7m"); // reverse video
            } else {
                frame.push_str("> ");
            }
        }
        frame.push_str(&line);
        if is_sel && use_color {
            frame.push_str("\x1b[0m");
        }
        frame.push_str("\x1b[K\r\n");
    }

    frame.push_str("\x1b[K\r\n");
    let hint_bot = "1 / 2 - quick select  |  Esc - back  |  Q - quit";
    let bx = center_x(cols, hint_bot.chars().count());
    frame.push_str(&" ".repeat(bx));
    if use_color {
        frame.push_str("\x1b[90m");
    }
    frame.push_str(hint_bot);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K");
    // Pad to the bottom so no visualizer leftovers remain on screen.
    top += lines;
    while top < rows {
        frame.push_str("\r\n\x1b[K");
        top += 1;
    }
}

/// Parameters for one visualizer frame.
pub struct Visualizer<'a> {
    /// Smoothed bar levels (0..1).
    pub bars: &'a [f32],
    /// Falling peak levels (0..1).
    pub peaks: &'a [f32],
    /// Terminal width in cells.
    pub cols: usize,
    /// Terminal height in cells.
    pub rows: usize,
    /// Bar width in cells (from the theme).
    pub bar_width: usize,
    /// Gap between bars in cells (from the theme).
    pub gap_width: usize,
    /// Whether ANSI colors are enabled.
    pub use_color: bool,
    /// Audio source label for the footer.
    pub source: &'a str,
    /// Active theme name for the footer.
    pub theme_name: &'a str,
}

/// Draws the spectrum bars into `frame` (fixed height, no clear, no flicker).
pub fn render_visualizer(frame: &mut String, v: &Visualizer) {
    let n_bars = v.bars.len();
    let area_h = v.rows.saturating_sub(FOOTER_HEIGHT).max(5);

    // Levels (0..1) mapped to rows, with peak rows rounded.
    let bar_h: Vec<f32> = v
        .bars
        .iter()
        .map(|b| (b * area_h as f32).clamp(0.0, area_h as f32))
        .collect();
    let cap_rows: Vec<usize> = v
        .peaks
        .iter()
        .map(|c| ((c * area_h as f32).round() as usize).min(area_h))
        .collect();

    let width = n_bars * v.bar_width + n_bars.saturating_sub(1) * v.gap_width;
    let pad_x = v.cols.saturating_sub(width) / 2;
    frame.clear();
    // Top row of the area; position the cursor once.
    let _ = write!(frame, "\x1b[H");
    let pad: String = " ".repeat(pad_x);
    for row in (0..area_h).rev() {
        frame.push_str(&pad);
        if v.use_color {
            frame.push_str(row_color(row, area_h));
        }
        for i in 0..n_bars {
            // Fractional height: the integer part is full blocks,
            // the fraction is 1/8-blocks for sub-cell smoothness.
            let fh = bar_h[i];
            let full = fh.floor() as usize;
            let frac = fh - full as f32;
            // Is there a peak marker exactly one cell above?
            let has_peak = cap_rows[i] == row + 1 && cap_rows[i] <= area_h && row >= full;
            if row < full {
                for _ in 0..v.bar_width {
                    frame.push('█');
                }
            } else if row == full {
                let idx = ((frac * 8.0).round() as usize).min(8);
                // 1/8-blocks, bottom to top.
                const PARTS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
                let ch = PARTS[idx];
                if ch == ' ' {
                    // Empty - may hold a peak marker.
                    if has_peak {
                        push_peak(frame, v.bar_width, v.use_color);
                        if v.use_color {
                            frame.push_str(row_color(row, area_h));
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
                // Peak marker exactly one cell above/at the top.
                push_peak(frame, v.bar_width, v.use_color);
                if v.use_color {
                    // Restore the row color for the following cells.
                    frame.push_str(row_color(row, area_h));
                }
            } else {
                for _ in 0..v.bar_width {
                    frame.push(' ');
                }
            }
            if v.gap_width > 0 && i + 1 < n_bars {
                if v.use_color {
                    frame.push_str("\x1b[0m");
                    frame.push_str(&" ".repeat(v.gap_width));
                    frame.push_str(row_color(row, area_h));
                } else {
                    frame.push_str(&" ".repeat(v.gap_width));
                }
            }
        }
        if v.use_color {
            frame.push_str("\x1b[0m");
        }
        // Erase leftovers of a previously longer line when shrinking.
        frame.push_str("\x1b[K\r\n");
    }
    // Footer: exactly FOOTER_HEIGHT lines for a fixed geometry.
    let short = truncate_chars(v.source, 30);
    let foot = format!("{short}  |  {}  |  T - themes  |  Q - quit", v.theme_name);
    let foot_w = foot.chars().count();
    let foot_x = v.cols.saturating_sub(foot_w) / 2;
    frame.push_str("\r\n");
    frame.push_str(&" ".repeat(foot_x));
    frame.push_str(&foot);
    frame.push_str("\x1b[K");
}

/// Pushes a cyan peak marker `bar_width` cells wide.
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
