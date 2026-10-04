use std::fmt::Write as _;

use crate::frame::center_x;

pub fn render_help(frame: &mut String, cols: usize, rows: usize, use_color: bool) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let lines = [
        "Keys",
        "",
        "Space / P      pause / resume",
        "L              toggle layout (Symmetric / Left-Right)",
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
