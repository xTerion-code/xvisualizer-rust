//! Keyboard input: theme menu navigation and global hotkeys.
//!
//! This module only translates key events into UI state changes.
//! It knows nothing about audio, FFT or rendering.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::source::{CaptureTarget, PlaybackStream, list_playback_streams};
use crate::theme::Theme;

/// UI state: which screen is shown and which theme is active/selected.
pub struct UiState {
    /// Theme currently applied to the visualizer.
    pub current: Theme,
    /// Theme highlighted in the theme menu.
    pub selected: usize,
    /// Whether the theme menu is shown instead of the visualizer.
    pub in_menu: bool,
    /// What is currently captured (system mix or one app).
    pub target: CaptureTarget,
    /// Playback streams listed in the source menu (refreshed on open).
    pub apps: Vec<PlaybackStream>,
    /// Row highlighted in the source menu (0 = system, 1.. = apps).
    pub source_selected: usize,
    /// Whether the source menu is shown instead of the visualizer.
    pub in_source_menu: bool,
}

/// Side-effect-free request from the key handler to the main loop.
/// Source switching touches PulseAudio and restarts capture, so `main`
/// applies these events after polling.
pub enum UiEvent {
    /// Capture the whole system mix again.
    SelectSystem,
    /// Solo-capture one app's playback stream.
    SelectStream(PlaybackStream),
}

impl UiState {
    /// Initial state: the app starts with the theme menu open.
    pub fn new(initial: Theme) -> Self {
        Self {
            current: initial,
            selected: initial.index(),
            in_menu: true,
            target: CaptureTarget::System,
            apps: Vec::new(),
            source_selected: 0,
            in_source_menu: false,
        }
    }

    /// Opens the menu, highlighting the active theme.
    fn open_menu(&mut self) {
        self.selected = self.current.index();
        self.in_source_menu = false;
        self.in_menu = true;
    }

    /// Moves the menu highlight (`dir`: +1 down/right, -1 up/left, wraps around).
    fn step(&mut self, dir: i32) {
        let n = Theme::all().len() as i32;
        self.selected = (self.selected as i32 + dir).rem_euclid(n) as usize;
    }

    /// Opens the source menu, refreshing the app list first.
    fn open_source_menu(&mut self) {
        self.apps = list_playback_streams();
        self.source_selected = self.target.menu_index(&self.apps);
        self.in_menu = false;
        self.in_source_menu = true;
    }

    /// Moves the source highlight, clamped to the available rows.
    fn source_step(&mut self, dir: i32) {
        let n = self.apps.len() as i32 + 1;
        self.source_selected = (self.source_selected as i32 + dir).rem_euclid(n) as usize;
    }

    /// Re-reads the app list while the source menu stays open.
    fn refresh_apps(&mut self) {
        self.apps = list_playback_streams();
        let max = self.apps.len();
        self.source_selected = self.source_selected.min(max);
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

enum Action {
    Nothing,
    Quit,
    Emit(UiEvent),
}

/// Arrows + Enter system for the source menu. Confirming emits
/// a switch request that `main` applies (PulseAudio rerouting).
fn handle_source_key(state: &mut UiState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => state.source_step(-1),
        KeyCode::Left | KeyCode::Char('h') => state.source_step(-1),
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => state.source_step(1),
        KeyCode::Right | KeyCode::Char('l') => state.source_step(1),
        KeyCode::Char(c @ '1'..='9') => {
            let idx = (c as usize) - ('1' as usize);
            if idx <= state.apps.len() {
                state.source_selected = idx;
            }
        }
        KeyCode::Char('r') | KeyCode::Char('R') => state.refresh_apps(),
        KeyCode::Enter | KeyCode::Char(' ') => {
            state.in_source_menu = false;
            if state.source_selected == 0 {
                state.target = CaptureTarget::System;
                return Action::Emit(UiEvent::SelectSystem);
            } else if let Some(app) = state.apps.get(state.source_selected - 1).cloned() {
                state.target = CaptureTarget::Stream(app.clone());
                return Action::Emit(UiEvent::SelectStream(app));
            }
        }
        KeyCode::Esc => {
            // Back without changing the source.
            state.source_selected = state.target.menu_index(&state.apps);
            state.in_source_menu = false;
        }
        _ => {}
    }
    Action::Nothing
}

/// Drains all pending key events without blocking the frame loop.
///
/// In raw mode Ctrl+C arrives as a key event rather than SIGINT,
/// so quitting is handled here by clearing `running`.
/// Source-switch requests are returned for `main` to apply.
pub fn poll_keys(state: &mut UiState, running: &AtomicBool) -> Vec<UiEvent> {
    let mut events = Vec::new();
    while event::poll(Duration::from_millis(0)).unwrap_or(false) {
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match handle_key(state, key) {
            Action::Nothing => {}
            Action::Quit => {
                running.store(false, Ordering::SeqCst);
                break;
            }
            Action::Emit(ev) => events.push(ev),
        }
    }
    events
}

fn is_quit(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c'))
        || matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
}

fn handle_key(state: &mut UiState, key: KeyEvent) -> Action {
    if is_quit(key) {
        return Action::Quit;
    }
    if state.in_source_menu {
        handle_source_key(state, key)
    } else if state.in_menu {
        handle_menu_key(state, key);
        Action::Nothing
    } else {
        handle_visualizer_key(state, key);
        Action::Nothing
    }
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
        // Open the source menu (system mix vs. one app).
        KeyCode::Char('s') | KeyCode::Char('S') => state.open_source_menu(),
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
