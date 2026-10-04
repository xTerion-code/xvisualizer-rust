use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::{ChoiceMenu, MenuOutcome, render_choice};

/// Spectrum layout: mirrored around the center (bass in the middle)
/// or plain left-to-right (bass on the left, treble on the right).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    #[default]
    Symmetric,
    LeftToRight,
}

impl LayoutMode {
    pub fn all() -> [LayoutMode; 2] {
        [LayoutMode::Symmetric, LayoutMode::LeftToRight]
    }

    pub fn index(self) -> usize {
        match self {
            LayoutMode::Symmetric => 0,
            LayoutMode::LeftToRight => 1,
        }
    }

    pub fn next(self) -> Self {
        match self {
            LayoutMode::Symmetric => LayoutMode::LeftToRight,
            LayoutMode::LeftToRight => LayoutMode::Symmetric,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            LayoutMode::Symmetric => "Symmetric",
            LayoutMode::LeftToRight => "Left-Right",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            LayoutMode::Symmetric => "mirrored, bass in the center",
            LayoutMode::LeftToRight => "bass left, treble right",
        }
    }
}

/// Center-symmetric spectrum mapping: bass in the middle, treble at the edges.
/// The analyzer produces `unique` bands (low -> high); the visualizer mirrors
/// them into `unique * 2 - 1` display bars so left and right stay symmetric.
pub fn unique_count(display_fit: usize) -> usize {
    display_fit.div_ceil(2).max(1)
}

pub fn display_count(unique: usize) -> usize {
    unique.saturating_mul(2).saturating_sub(1).max(1)
}

/// Display position `d` (0 = left edge) to spectrum index (0 = bass).
pub fn spectrum_index(display_idx: usize, unique: usize) -> usize {
    if unique <= 1 {
        return 0;
    }
    let center = unique - 1;
    (display_idx as isize - center as isize).unsigned_abs()
}

pub struct LayoutMenu {
    pub current: LayoutMode,
    pub selected: usize,
    pub open: bool,
}

impl LayoutMenu {
    pub fn new() -> Self {
        Self {
            current: LayoutMode::default(),
            selected: LayoutMode::default().index(),
            open: false,
        }
    }

    pub fn open(&mut self) {
        self.selected = self.current.index();
        self.open = true;
    }

    pub fn step(&mut self, dir: i32) {
        let n = LayoutMode::all().len() as i32;
        self.selected = (self.selected as i32 + dir).rem_euclid(n) as usize;
    }

    pub fn toggle(&mut self) {
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
                if idx < LayoutMode::all().len() {
                    self.selected = idx;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.current = LayoutMode::all()[self.selected];
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
    current: LayoutMode,
    use_color: bool,
) {
    let modes = LayoutMode::all();
    let options: Vec<(String, bool)> = modes
        .iter()
        .map(|m| (format!("{}  [{}]", m.name(), m.desc()), *m == current))
        .collect();
    render_choice(
        frame,
        cols,
        rows,
        &ChoiceMenu {
            title: "Setup 3/4 - Select layout",
            options: &options,
            selected,
            quick_hint: "1 / 2 - quick select",
        },
        use_color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_counts_round_trip() {
        assert_eq!(unique_count(7), 4);
        assert_eq!(display_count(4), 7);
        assert_eq!(display_count(1), 1);
    }

    #[test]
    fn bass_sits_in_the_center() {
        // unique=3 -> display [2,1,0,1,2], center is bass.
        assert_eq!(spectrum_index(0, 3), 2);
        assert_eq!(spectrum_index(2, 3), 0);
        assert_eq!(spectrum_index(4, 3), 2);
    }

    #[test]
    fn single_band_maps_to_zero() {
        assert_eq!(spectrum_index(5, 1), 0);
        assert_eq!(spectrum_index(0, 0), 0);
    }
}
