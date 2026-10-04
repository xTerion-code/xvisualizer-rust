use std::fmt::Write as _;

use crossterm::event::{KeyCode, KeyEvent};

use crate::frame::center_x;
use crate::source::{CaptureTarget, PlaybackStream, list_playback_streams};
use crate::ui::{MenuOutcome, UiEvent};

pub struct SourceMenu {
    pub target: CaptureTarget,
    pub apps: Vec<PlaybackStream>,
    pub selected: usize,
    pub open: bool,
}

impl SourceMenu {
    pub fn new() -> Self {
        Self {
            target: CaptureTarget::System,
            apps: Vec::new(),
            selected: 0,
            open: false,
        }
    }

    pub fn open(&mut self) {
        self.apps = list_playback_streams();
        self.selected = self.target.menu_index(&self.apps);
        self.open = true;
    }

    fn step(&mut self, dir: i32) {
        let n = self.apps.len() as i32 + 1;
        self.selected = (self.selected as i32 + dir).rem_euclid(n) as usize;
    }

    fn refresh(&mut self) {
        self.apps = list_playback_streams();
        let max = self.apps.len();
        self.selected = self.selected.min(max);
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> (MenuOutcome, Option<UiEvent>) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => self.step(-1),
            KeyCode::Left | KeyCode::Char('h') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.step(1),
            KeyCode::Right | KeyCode::Char('l') => self.step(1),
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx <= self.apps.len() {
                    self.selected = idx;
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => self.refresh(),
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.open = false;
                if self.selected == 0 {
                    self.target = CaptureTarget::System;
                    return (MenuOutcome::Confirmed, Some(UiEvent::SelectSystem));
                } else if let Some(app) = self.apps.get(self.selected - 1).cloned() {
                    self.target = CaptureTarget::Stream(app.clone());
                    return (MenuOutcome::Confirmed, Some(UiEvent::SelectStream(app)));
                }
                return (MenuOutcome::Confirmed, None);
            }
            KeyCode::Esc => {
                self.selected = self.target.menu_index(&self.apps);
                self.open = false;
                return (MenuOutcome::Cancelled, None);
            }
            _ => {}
        }
        (MenuOutcome::StillOpen, None)
    }
}

pub fn render(
    frame: &mut String,
    cols: usize,
    rows: usize,
    menu: &SourceMenu,
    setup: bool,
    use_color: bool,
) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let current_input = match &menu.target {
        CaptureTarget::Stream(s) => Some(s.input),
        CaptureTarget::System => None,
    };

    let mut lines: Vec<String> = Vec::with_capacity(menu.apps.len() + 5);
    lines.push(if setup {
        "Setup 4/4 - Select capture source".to_string()
    } else {
        "Select capture source".to_string()
    });
    lines.push("Up/Down - select  |  Enter - capture  |  R - refresh".to_string());
    lines.push(String::new());
    lines.push(menu_row(
        0,
        menu.selected,
        "System",
        "everything you hear",
        matches!(menu.target, CaptureTarget::System),
    ));
    for (i, app) in menu.apps.iter().enumerate() {
        lines.push(menu_row(
            i + 1,
            menu.selected,
            &app.app,
            &app.stream,
            current_input == Some(app.input),
        ));
    }
    if menu.apps.is_empty() {
        lines.push("(no apps playing audio right now)".to_string());
    }
    lines.push(String::new());
    lines.push(if setup {
        "Esc - skip setup  |  Q - quit".to_string()
    } else {
        "Esc - back  |  Q - quit".to_string()
    });

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
