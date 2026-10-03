// Newlines are always `\r\n`: raw mode disables `\n` -> `\r\n` translation,
// so a bare `\n` would cause the staircase effect.

use std::fmt::Write as _;

use crate::source::{CaptureTarget, PlaybackStream};
use crate::theme::{ColorMode, Theme};

const FOOTER_HEIGHT: usize = 2;

pub fn truncate_chars(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        return s;
    }
    let idx = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    &s[..idx]
}

fn row_color_height(row: usize, area_h: usize) -> &'static str {
    let ratio = row as f32 / area_h.max(1) as f32;
    if ratio >= 0.85 {
        "\x1b[31m"
    } else if ratio >= 0.6 {
        "\x1b[33m"
    } else {
        "\x1b[32m"
    }
}

fn freq_color(idx: usize, n_bars: usize) -> &'static str {
    let ratio = idx as f32 / n_bars.max(1) as f32;
    if ratio < 0.2 {
        "\x1b[31m"
    } else if ratio < 0.4 {
        "\x1b[33m"
    } else if ratio < 0.6 {
        "\x1b[32m"
    } else if ratio < 0.8 {
        "\x1b[36m"
    } else {
        "\x1b[35m"
    }
}

fn bar_color(
    row: usize,
    area_h: usize,
    bar_idx: usize,
    n_bars: usize,
    mode: ColorMode,
) -> &'static str {
    match mode {
        ColorMode::Height => row_color_height(row, area_h),
        ColorMode::Frequency => freq_color(bar_idx, n_bars),
        ColorMode::Mono => "\x1b[36m",
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

pub fn render_help(frame: &mut String, cols: usize, rows: usize, use_color: bool) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let lines = [
        "Keys",
        "",
        "Space / P      pause / resume",
        "+ / -          sensitivity up / down",
        "G              reset sensitivity to 1.0x",
        "A              toggle auto-gain (AUTO / MAN)",
        "C              cycle bar color (Height / Frequency / Mono)",
        "T / Tab        theme menu",
        "S              capture source menu",
        "1 / 2          quick theme switch",
        "H / ? / F1     this help",
        "Q / Ctrl+C     quit",
        "",
        "Esc - back",
    ];
    // Exactly `rows` lines: no trailing newline past the bottom, otherwise
    // the 120 Hz redraw would scroll and smear old rows over the screen.
    let top = rows.saturating_sub(lines.len()) / 2;
    for r in 0..rows {
        if r > 0 {
            frame.push_str("\r\n");
        }
        frame.push_str("\x1b[K");
        if r < top || r >= top + lines.len() {
            continue;
        }
        let line = lines[r - top];
        if line.is_empty() {
            continue;
        }
        let is_title = r == top;
        let x = center_x(cols, line.chars().count());
        frame.push_str(&" ".repeat(x));
        if is_title && use_color {
            frame.push_str("\x1b[1;36m");
        }
        frame.push_str(line);
        if is_title && use_color {
            frame.push_str("\x1b[0m");
        }
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
