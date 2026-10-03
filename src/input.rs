use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::source::{CaptureTarget, PlaybackStream, list_playback_streams};
use crate::theme::{ColorMode, Theme};

pub struct UiState {
    pub current: Theme,
    pub selected: usize,
    pub in_menu: bool,
    pub target: CaptureTarget,
    pub apps: Vec<PlaybackStream>,
    pub source_selected: usize,
    pub in_source_menu: bool,
    pub paused: bool,
    pub color: ColorMode,
    pub in_help: bool,
}

// Source switching reroutes PulseAudio and restarts capture, so the handler
// only returns a request and `main` applies it.
pub enum UiEvent {
    SelectSystem,
    SelectStream(PlaybackStream),
    GainUp,
    GainDown,
    GainReset,
    ToggleAutoGain,
}

impl UiState {
    pub fn new(initial: Theme) -> Self {
        Self {
            current: initial,
            selected: initial.index(),
            // App starts with the theme menu open.
            in_menu: true,
            target: CaptureTarget::System,
            apps: Vec::new(),
            source_selected: 0,
            in_source_menu: false,
            paused: false,
            color: ColorMode::default(),
            in_help: false,
        }
    }

    fn open_menu(&mut self) {
        self.selected = self.current.index();
        self.in_source_menu = false;
        self.in_help = false;
        self.in_menu = true;
    }

    fn step(&mut self, dir: i32) {
        let n = Theme::all().len() as i32;
        self.selected = (self.selected as i32 + dir).rem_euclid(n) as usize;
    }

    fn open_source_menu(&mut self) {
        self.apps = list_playback_streams();
        self.source_selected = self.target.menu_index(&self.apps);
        self.in_menu = false;
        self.in_help = false;
        self.in_source_menu = true;
    }

    fn source_step(&mut self, dir: i32) {
        let n = self.apps.len() as i32 + 1;
        self.source_selected = (self.source_selected as i32 + dir).rem_euclid(n) as usize;
    }

    fn refresh_apps(&mut self) {
        self.apps = list_playback_streams();
        let max = self.apps.len();
        self.source_selected = self.source_selected.min(max);
    }

    fn confirm(&mut self) {
        self.current = Theme::all()[self.selected];
        self.in_menu = false;
    }

    fn cancel(&mut self) {
        self.selected = self.current.index();
        self.in_menu = false;
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    fn cycle_color(&mut self) {
        self.color = self.color.next();
    }

    fn toggle_help(&mut self) {
        self.in_help = !self.in_help;
    }
}

enum Action {
    Nothing,
    Quit,
    Emit(UiEvent),
}

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
            state.source_selected = state.target.menu_index(&state.apps);
            state.in_source_menu = false;
        }
        _ => {}
    }
    Action::Nothing
}

/// Drains pending keys without blocking the frame loop.
///
/// In raw mode Ctrl+C arrives as a key event, not SIGINT.
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
    (key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')))
        || matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
}

fn handle_key(state: &mut UiState, key: KeyEvent) -> Action {
    if is_quit(key) {
        return Action::Quit;
    }
    if state.in_help {
        handle_help_key(state, key);
        return Action::Nothing;
    }
    if state.in_source_menu {
        handle_source_key(state, key)
    } else if state.in_menu {
        handle_menu_key(state, key);
        Action::Nothing
    } else {
        handle_visualizer_key(state, key)
    }
}

fn handle_help_key(state: &mut UiState, key: KeyEvent) {
    match key.code {
        // Any of these closes the overlay; visualizer keys stay inert behind it.
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => state.in_help = false,
        KeyCode::Char('?') | KeyCode::Char('h') | KeyCode::Char('H') => state.in_help = false,
        KeyCode::F(1) => state.in_help = false,
        _ => {}
    }
}

fn handle_menu_key(state: &mut UiState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => state.step(-1),
        KeyCode::Left | KeyCode::Char('h') => state.step(-1),
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => state.step(1),
        KeyCode::Right | KeyCode::Char('l') => state.step(1),
        KeyCode::Char(c) if ('1'..='9').contains(&c) => {
            let idx = (c as usize) - ('1' as usize);
            if idx < Theme::all().len() {
                state.selected = idx;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => state.confirm(),
        KeyCode::Esc => state.cancel(),
        _ => {}
    }
    Action::Nothing
}

fn handle_visualizer_key(state: &mut UiState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('t')
        | KeyCode::Char('T')
        | KeyCode::Char('m')
        | KeyCode::Char('M')
        | KeyCode::Tab
        | KeyCode::F(2) => state.open_menu(),
        KeyCode::Char('s') | KeyCode::Char('S') => state.open_source_menu(),
        KeyCode::Char(' ') | KeyCode::Char('p') | KeyCode::Char('P') => state.toggle_pause(),
        KeyCode::Char('+') | KeyCode::Char('=') => return Action::Emit(UiEvent::GainUp),
        KeyCode::Char('-') | KeyCode::Char('_') => return Action::Emit(UiEvent::GainDown),
        KeyCode::Char('g') | KeyCode::Char('G') => return Action::Emit(UiEvent::GainReset),
        KeyCode::Char('a') | KeyCode::Char('A') => return Action::Emit(UiEvent::ToggleAutoGain),
        KeyCode::Char('c') | KeyCode::Char('C') => state.cycle_color(),
        KeyCode::Char('?') | KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::F(1) => {
            state.toggle_help()
        }
        // Arrows open the menu too; the pressed arrow already moves
        // the highlight so the key feels responsive.
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
            state.open_menu();
            match key.code {
                KeyCode::Down | KeyCode::Right => state.step(1),
                _ => state.step(-1),
            }
        }
        KeyCode::Char(c) if ('1'..='9').contains(&c) => {
            let idx = (c as usize) - ('1' as usize);
            if idx < Theme::all().len() {
                state.current = Theme::all()[idx];
            }
        }
        _ => {}
    }
    Action::Nothing
}
