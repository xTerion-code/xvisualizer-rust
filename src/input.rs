use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::help;
use crate::ui::{MenuOutcome, UiEvent, UiState};

enum Action {
    Nothing,
    Quit,
    Emit(UiEvent),
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
        help::handle_key(&mut state.in_help, key);
        return Action::Nothing;
    }
    if state.source.open {
        let (outcome, event) = state.source.handle_key(key);
        if let Some(ev) = event {
            return Action::Emit(ev);
        }
        if !matches!(outcome, MenuOutcome::StillOpen) {
            state.setup = false;
        }
        return Action::Nothing;
    }
    if state.theme.open {
        match state.theme.handle_key(key) {
            MenuOutcome::Confirmed if state.setup => open_color(state),
            MenuOutcome::Cancelled => state.setup = false,
            _ => {}
        }
        return Action::Nothing;
    }
    if state.color.open {
        match state.color.handle_key(key) {
            MenuOutcome::Confirmed if state.setup => open_layout(state),
            MenuOutcome::Cancelled => state.setup = false,
            _ => {}
        }
        return Action::Nothing;
    }
    if state.layout.open {
        match state.layout.handle_key(key) {
            MenuOutcome::Confirmed if state.setup => open_source(state),
            MenuOutcome::Cancelled => state.setup = false,
            _ => {}
        }
        return Action::Nothing;
    }
    handle_visualizer_key(state, key)
}

fn open_theme(state: &mut UiState) {
    state.theme.open();
    state.source.open = false;
    state.color.open = false;
    state.layout.open = false;
    state.in_help = false;
}

fn open_source(state: &mut UiState) {
    state.source.open();
    state.theme.open = false;
    state.color.open = false;
    state.layout.open = false;
    state.in_help = false;
}

fn open_color(state: &mut UiState) {
    state.color.open();
    state.theme.open = false;
    state.source.open = false;
    state.layout.open = false;
    state.in_help = false;
}

fn open_layout(state: &mut UiState) {
    state.layout.open();
    state.theme.open = false;
    state.source.open = false;
    state.color.open = false;
    state.in_help = false;
}

fn handle_visualizer_key(state: &mut UiState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('t')
        | KeyCode::Char('T')
        | KeyCode::Char('m')
        | KeyCode::Char('M')
        | KeyCode::Tab
        | KeyCode::F(2) => open_theme(state),
        KeyCode::Char('s') | KeyCode::Char('S') => open_source(state),
        KeyCode::Char(' ') | KeyCode::Char('p') | KeyCode::Char('P') => state.toggle_pause(),
        KeyCode::Char('+') | KeyCode::Char('=') => return Action::Emit(UiEvent::GainUp),
        KeyCode::Char('-') | KeyCode::Char('_') => return Action::Emit(UiEvent::GainDown),
        KeyCode::Char('g') | KeyCode::Char('G') => return Action::Emit(UiEvent::GainReset),
        KeyCode::Char('c') | KeyCode::Char('C') => state.color.cycle(),
        KeyCode::Char('l') | KeyCode::Char('L') => state.layout.toggle(),
        KeyCode::Char('?') | KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::F(1) => {
            state.toggle_help()
        }
        // Arrows open the menu too; the pressed arrow already moves
        // the highlight so the key feels responsive.
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
            open_theme(state);
            match key.code {
                KeyCode::Down | KeyCode::Right => state.theme.step(1),
                _ => state.theme.step(-1),
            }
        }
        KeyCode::Char(c) if ('1'..='9').contains(&c) => {
            let idx = (c as usize) - ('1' as usize);
            state.theme.quick(idx);
        }
        _ => {}
    }
    Action::Nothing
}
