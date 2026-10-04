use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::source::CaptureTarget;
use crate::theme::Theme;
use crate::ui::{UiEvent, UiState};

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
        KeyCode::Char('c') | KeyCode::Char('C') => state.cycle_color(),
        KeyCode::Char('v') | KeyCode::Char('V') => state.toggle_layout(),
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
