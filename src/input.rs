//! Keyboard input: theme menu navigation and global hotkeys.
//!
//! This module only translates key events into UI state changes.
//! It knows nothing about audio, FFT or rendering.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::theme::Theme;

/// UI state: which screen is shown and which theme is active/selected.
pub struct UiState {
    /// Theme currently applied to the visualizer.
    pub current: Theme,
    /// Theme highlighted in the menu.
    pub selected: usize,
    /// Whether the theme menu is shown instead of the visualizer.
    pub in_menu: bool,
}

impl UiState {
    /// Initial state: the app starts with the theme menu open.
    pub fn new(initial: Theme) -> Self {
        Self {
            current: initial,
            selected: initial.index(),
            in_menu: true,
        }
    }

    /// Opens the menu, highlighting the active theme.
    fn open_menu(&mut self) {
        self.selected = self.current.index();
        self.in_menu = true;
    }

    /// Moves the menu highlight (`dir`: +1 down/right, -1 up/left, wraps around).
    fn step(&mut self, dir: i32) {
        let n = Theme::all().len() as i32;
        self.selected = (self.selected as i32 + dir).rem_euclid(n) as usize;
    }

    /// Applies the highlighted theme and returns to the visualizer.
    fn confirm(&mut self) {
        self.current = Theme::all()[self.selected];
        self.in_menu = false;
    }

    /// Returns to the visualizer without changing the theme.
    fn cancel(&mut self) {
        self.selected = self.current.index();
        self.in_menu = false;
    }
}

#[derive(PartialEq, Eq)]
enum Action {
    Nothing,
    Quit,
}

/// Drains all pending key events without blocking the frame loop.
///
/// In raw mode Ctrl+C arrives as a key event rather than SIGINT,
/// so quitting is handled here by clearing `running`.
pub fn poll_keys(state: &mut UiState, running: &AtomicBool) {
    while event::poll(Duration::from_millis(0)).unwrap_or(false) {
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if handle_key(state, key) == Action::Quit {
            running.store(false, Ordering::SeqCst);
            break;
        }
    }
}

fn is_quit(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c'))
        || matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
}

fn handle_key(state: &mut UiState, key: KeyEvent) -> Action {
    if is_quit(key) {
        return Action::Quit;
    }
    if state.in_menu {
        handle_menu_key(state, key);
    } else {
        handle_visualizer_key(state, key);
    }
    Action::Nothing
}

/// Arrows + Enter system for the theme menu.
fn handle_menu_key(state: &mut UiState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => state.step(-1),
        KeyCode::Left | KeyCode::Char('h') => state.step(-1),
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => state.step(1),
        KeyCode::Right | KeyCode::Char('l') => state.step(1),
        KeyCode::Char('1') => state.selected = 0,
        KeyCode::Char('2') => state.selected = 1,
        KeyCode::Enter | KeyCode::Char(' ') => state.confirm(),
        KeyCode::Esc => state.cancel(),
        _ => {}
    }
    Action::Nothing
}

fn handle_visualizer_key(state: &mut UiState, key: KeyEvent) -> Action {
    match key.code {
        // Open the theme menu.
        KeyCode::Char('t')
        | KeyCode::Char('T')
        | KeyCode::Char('m')
        | KeyCode::Char('M')
        | KeyCode::Tab
        | KeyCode::F(2) => state.open_menu(),
        // Arrows also open the menu, so arrow selection is reachable
        // straight from the visualizer. The pressed arrow already moves
        // the highlight so the key feels responsive.
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
            state.open_menu();
            match key.code {
                KeyCode::Down | KeyCode::Right => state.step(1),
                _ => state.step(-1),
            }
        }
        // Quick theme switch without the menu.
        KeyCode::Char('1') => state.current = Theme::Classic,
        KeyCode::Char('2') => state.current = Theme::Solid,
        _ => {}
    }
    Action::Nothing
}
