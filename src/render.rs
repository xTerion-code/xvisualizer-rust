// Newlines are always `\r\n`: raw mode disables `\n` -> `\r\n` translation,
// so a bare `\n` would cause the staircase effect.

use std::fmt::Write as _;

use crate::source::{CaptureTarget, PlaybackStream};
use crate::theme::Theme;

const FOOTER_HEIGHT: usize = 2;

pub fn truncate_chars(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        return s;
    }
    let idx = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    &s[..idx]
}

fn row_color(row: usize, area_h: usize) -> &'static str {
    let ratio = row as f32 / area_h.max(1) as f32;
    if ratio >= 0.85 {
        "\x1b[31m"
    } else if ratio >= 0.6 {
        "\x1b[33m"
    } else {
        "\x1b[32m"
    }
}

fn center_x(cols: usize, width: usize) -> usize {
    cols.saturating_sub(width) / 2
}

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
                frame.push_str("\x1b[7m");
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
    // Clear any visualizer leftovers below the menu.
    top += lines;
    while top < rows {
        frame.push_str("\r\n\x1b[K");
        top += 1;
    }
}

pub fn render_source_menu(
    frame: &mut String,
    cols: usize,
    rows: usize,
    apps: &[PlaybackStream],
    selected: usize,
    target: &CaptureTarget,
    use_color: bool,
) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let current_input = match target {
        CaptureTarget::Stream(s) => Some(s.input),
        CaptureTarget::System => None,
    };

    let mut lines: Vec<String> = Vec::with_capacity(apps.len() + 5);
    lines.push("Select capture source".to_string());
    lines.push("Up/Down - select  |  Enter - capture  |  R - refresh".to_string());
    lines.push(String::new());
    lines.push(menu_row(
        0,
        selected,
        "System",
        "everything you hear",
        matches!(target, CaptureTarget::System),
    ));
    for (i, app) in apps.iter().enumerate() {
        lines.push(menu_row(
            i + 1,
            selected,
            &app.app,
            &app.stream,
            current_input == Some(app.input),
        ));
    }
    if apps.is_empty() {
        lines.push("(no apps playing audio right now)".to_string());
    }
    lines.push(String::new());
    lines.push("Esc - back  |  Q - quit".to_string());

    let mut top = rows.saturating_sub(lines.len()) / 2;
    for _ in 0..top {
        frame.push_str("\x1b[K\r\n");
    }
    for (idx, line) in lines.iter().enumerate() {
        let is_title = idx == 0;
        let is_hint = idx == 1 || idx == lines.len() - 1;
        let is_option = !is_title && !is_hint && !line.is_empty() && !line.starts_with('(');
        if line.is_empty() {
            frame.push_str("\x1b[K\r\n");
            continue;
        }
        let x = center_x(cols, line.chars().count());
        frame.push_str(&" ".repeat(x));
        if is_title && use_color {
            frame.push_str("\x1b[1;36m");
        } else if is_hint && use_color {
            frame.push_str("\x1b[90m");
        } else if is_option && line.starts_with("> ") && use_color {
            frame.push_str("\x1b[7m");
        }
        frame.push_str(line);
        if (is_title || is_hint || (is_option && line.starts_with("> "))) && use_color {
            frame.push_str("\x1b[0m");
        }
        if idx + 1 < lines.len() {
            frame.push_str("\x1b[K\r\n");
        } else {
            frame.push_str("\x1b[K");
        }
    }
    // Clear any visualizer leftovers below the menu.
    top += lines.len();
    while top < rows {
        frame.push_str("\r\n\x1b[K");
        top += 1;
    }
}

fn menu_row(idx: usize, selected: usize, name: &str, detail: &str, is_current: bool) -> String {
    let marker = if idx == selected { "> " } else { "  " };
    let cur = if is_current { "  (current)" } else { "" };
    if detail.is_empty() {
        format!("{marker}{name}{cur}")
    } else {
        format!("{marker}{name} - {detail}{cur}")
    }
}
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
}

pub fn render_visualizer(frame: &mut String, v: &Visualizer) {
    let n_bars = v.bars.len();
    let area_h = v.rows.saturating_sub(FOOTER_HEIGHT).max(5);

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
    let _ = write!(frame, "\x1b[H");
    let pad: String = " ".repeat(pad_x);
    for row in (0..area_h).rev() {
        frame.push_str(&pad);
        if v.use_color {
            frame.push_str(row_color(row, area_h));
        }
        for i in 0..n_bars {
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
                const PARTS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
                let ch = PARTS[idx];
                if ch == ' ' {
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
                push_peak(frame, v.bar_width, v.use_color);
                if v.use_color {
                    // Peak uses its own color; restore the row color after it.
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
        // Clear leftovers when shrinking to a narrower frame.
        frame.push_str("\x1b[K\r\n");
    }
    let foot = match v.notice {
        Some(msg) => truncate_chars(msg, 60).to_string(),
        None => {
            let short = truncate_chars(v.source, 30);
            format!(
                "{short}  |  {}  |  T - themes  |  S - source  |  Q - quit",
                v.theme_name
            )
        }
    };
    let foot_w = foot.chars().count();
    let foot_x = v.cols.saturating_sub(foot_w) / 2;
    frame.push_str("\r\n");
    frame.push_str(&" ".repeat(foot_x));
    frame.push_str(&foot);
    frame.push_str("\x1b[K");
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
