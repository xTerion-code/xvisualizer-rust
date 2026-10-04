use crossterm::event::{KeyCode, KeyEvent};

use crate::choice_menu::{ChoiceMenu, render_choice};
use crate::color::ColorMode;
use crate::ui::MenuOutcome;

pub struct ColorMenu {
    pub current: ColorMode,
    pub selected: usize,
    pub open: bool,
}

impl ColorMenu {
    pub fn new() -> Self {
        Self {
            current: ColorMode::default(),
            selected: ColorMode::default().index(),
            open: false,
        }
    }

    pub fn open(&mut self) {
        self.selected = self.current.index();
        self.open = true;
    }

    pub fn step(&mut self, dir: i32) {
        let n = ColorMode::all().len() as i32;
        self.selected = (self.selected as i32 + dir).rem_euclid(n) as usize;
    }

    pub fn cycle(&mut self) {
        self.current = self.current.next();
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> MenuOutcome {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => self.step(-1),
            KeyCode::Left | KeyCode::Char('h') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.step(1),
            KeyCode::Right | KeyCode::Char('l') => self.step(1),
            KeyCode::Char(c) if ('1'..='9').contains(&c) => {
                let idx = (c as usize) - ('1' as usize);
                if idx < ColorMode::all().len() {
                    self.selected = idx;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.current = ColorMode::all()[self.selected];
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
    current: ColorMode,
    use_color: bool,
) {
    let modes = ColorMode::all();
    let options: Vec<(String, bool)> = modes
        .iter()
        .map(|m| (format!("{}  [{}]", m.name(), m.desc()), *m == current))
        .collect();
    render_choice(
        frame,
        cols,
        rows,
        &ChoiceMenu {
            title: "Setup 2/4 - Select bar color",
            options: &options,
            selected,
            quick_hint: "1 / 2 / 3 - quick select",
        },
        use_color,
    );
}
