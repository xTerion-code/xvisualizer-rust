use std::fmt::Write as _;

use crossterm::event::{KeyCode, KeyEvent};

use crate::util::center_x;

pub fn handle_key(open: &mut bool, key: KeyEvent) {
    match key.code {
        // Any of these closes the overlay; visualizer keys stay inert behind it.
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => *open = false,
        KeyCode::Char('?') | KeyCode::Char('h') | KeyCode::Char('H') => *open = false,
        KeyCode::F(1) => *open = false,
        _ => {}
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
        "L              toggle layout (Symmetric / Left-Right)",
        "C              cycle bar color (Height / Frequency / Mono)",
        "T / Tab / M    theme menu (arrows open it too)",
        "S              capture source menu",
        "1 / 2          quick theme switch",
        "H / ? / F1     this help",
        "Q / Ctrl+C     quit",
        "",
        "Esc - back",
    ];
    // Exactly `rows` lines: no trailing newline past the bottom, otherwise
    // the 120 Hz redraw would scroll and smear old rows over the screen.
    // Center the block as a whole: per-line centering would stagger
    // the description column because lines differ in width.
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let x = center_x(cols, width);
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
        // Title stays centered on its own; options share one left edge.
        let pad = if is_title {
            center_x(cols, line.chars().count())
        } else {
            x
        };
        frame.push_str(&" ".repeat(pad));
        if is_title && use_color {
            frame.push_str("\x1b[1;36m");
        }
        frame.push_str(line);
        if is_title && use_color {
            frame.push_str("\x1b[0m");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_options_share_one_left_edge() {
        let mut frame = String::new();
        render_help(&mut frame, 80, 24, false);
        let starts: Vec<usize> = frame
            .split("\r\n")
            .map(|l| l.trim_start_matches("\x1b[H").trim_start_matches("\x1b[K"))
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.chars().take_while(|c| *c == ' ').count())
            .collect();
        // First line is the centered title; the rest form one left-aligned block.
        assert!(starts.len() > 2);
        for s in &starts[1..] {
            assert_eq!(*s, starts[1]);
        }
    }
}
