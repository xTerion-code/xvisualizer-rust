use std::fmt::Write as _;

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::MenuOutcome;
use crate::util::center_x;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Classic,
    Solid,
}

impl Theme {
    pub fn all() -> [Theme; 2] {
        [Theme::Classic, Theme::Solid]
    }

    pub fn index(self) -> usize {
        match self {
            Theme::Classic => 0,
            Theme::Solid => 1,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Theme::Classic => "Classic",
            Theme::Solid => "Solid",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Theme::Classic => "thin bars with gaps",
            Theme::Solid => "wide bars without gaps",
        }
    }

    pub fn dims(self) -> (usize, usize) {
        match self {
            Theme::Classic => (1, 1),
            Theme::Solid => (2, 0),
        }
    }

    pub fn from_str(s: &str) -> Option<Theme> {
        match s.to_lowercase().as_str() {
            "classic" | "1" => Some(Theme::Classic),
            "solid" | "bold" | "fat" | "2" => Some(Theme::Solid),
            _ => None,
        }
    }

    pub fn fit_bar_count(self, cols: usize, max: usize) -> usize {
        let (bar_w, gap_w) = self.dims();
        let cell = (bar_w + gap_w).max(1);
        let mut n = ((cols.saturating_sub(4) + gap_w) / cell).clamp(1, max);
        // Shrink until the bars actually fit; guards very narrow windows.
        while n > 1 && (n * bar_w + n.saturating_sub(1) * gap_w) > cols {
            n -= 1;
        }
        n
    }
}

pub struct ThemeMenu {
    pub current: Theme,
    pub selected: usize,
    pub open: bool,
}

impl ThemeMenu {
    pub fn new(initial: Theme) -> Self {
        // App starts with the setup wizard open (theme first).
        Self {
            current: initial,
            selected: initial.index(),
            open: true,
        }
    }

    pub fn open(&mut self) {
        self.selected = self.current.index();
        self.open = true;
    }

    pub fn step(&mut self, dir: i32) {
        let n = Theme::all().len() as i32;
        self.selected = (self.selected as i32 + dir).rem_euclid(n) as usize;
    }

    pub fn quick(&mut self, idx: usize) {
        if idx < Theme::all().len() {
            self.current = Theme::all()[idx];
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> MenuOutcome {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => self.step(-1),
            KeyCode::Left | KeyCode::Char('h') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.step(1),
            KeyCode::Right | KeyCode::Char('l') => self.step(1),
            KeyCode::Char(c) if ('1'..='9').contains(&c) => {
                let idx = (c as usize) - ('1' as usize);
                if idx < Theme::all().len() {
                    self.selected = idx;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.current = Theme::all()[self.selected];
                self.open = false;
                return MenuOutcome::Confirmed;
            }
            KeyCode::Esc => {
                self.selected = self.current.index();
                self.open = false;
                return MenuOutcome::Cancelled;
            }
            _ => {}
        }
        MenuOutcome::StillOpen
    }
}

pub fn render(
    frame: &mut String,
    cols: usize,
    rows: usize,
    selected: usize,
    current: Theme,
    setup: bool,
    use_color: bool,
) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let lines = 9usize;
    let mut top = rows.saturating_sub(lines) / 2;
    for _ in 0..top {
        frame.push_str("\x1b[K\r\n");
    }
    let title = if setup {
        "Setup 1/4 - Select theme"
    } else {
        "Select theme"
    };
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
    let hint_bot = if setup {
        "1 / 2 - quick select  |  Enter - next  |  Esc - skip setup  |  Q - quit"
    } else {
        "1 / 2 - quick select  |  Esc - back  |  Q - quit"
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_leaves_room_for_margins() {
        // Classic cell is 2 wide; 80 cols minus 4 margin fit 38 bars.
        assert_eq!(Theme::Classic.fit_bar_count(80, 64), 38);
    }

    #[test]
    fn solid_uses_full_width() {
        assert_eq!(Theme::Solid.fit_bar_count(80, 64), 38);
    }

    #[test]
    fn narrow_window_shrinks_to_fit() {
        assert_eq!(Theme::Classic.fit_bar_count(3, 64), 1);
        assert_eq!(Theme::Solid.fit_bar_count(1, 64), 1);
    }
}
