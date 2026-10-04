use std::fmt::Write as _;

use crate::color::ColorMenu;
use crate::layout::LayoutMenu;
use crate::source::PlaybackStream;
use crate::source::SourceMenu;
use crate::theme::Theme;
use crate::theme::ThemeMenu;
use crate::util::center_x;

// Composition root for screen state: each menu owns its own
// state in its file, `main` owns this struct.
pub struct UiState {
    pub theme: ThemeMenu,
    pub source: SourceMenu,
    pub color: ColorMenu,
    pub layout: LayoutMenu,
    pub paused: bool,
    pub in_help: bool,
    // True until the startup wizard (theme -> color -> layout -> source)
    // completes or is skipped with Esc.
    pub setup: bool,
}

// Source switching reroutes PulseAudio and restarts capture, so the handler
// only returns a request and `main` applies it.
pub enum UiEvent {
    SelectSystem,
    SelectStream(PlaybackStream),
    GainUp,
    GainDown,
    GainReset,
}

pub enum MenuOutcome {
    StillOpen,
    Confirmed,
    Cancelled,
}

impl UiState {
    pub fn new(initial: Theme) -> Self {
        Self {
            theme: ThemeMenu::new(initial),
            source: SourceMenu::new(),
            color: ColorMenu::new(),
            layout: LayoutMenu::new(),
            paused: false,
            in_help: false,
            setup: true,
        }
    }

    pub(crate) fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub(crate) fn toggle_help(&mut self) {
        self.in_help = !self.in_help;
    }
}

// Generic option-list screen backing the setup wizard steps
// that offer a fixed set of choices (color, layout).
pub(crate) struct ChoiceMenu<'a> {
    pub(crate) title: &'a str,
    pub(crate) options: &'a [(String, bool)],
    pub(crate) selected: usize,
    pub(crate) quick_hint: &'a str,
}

pub(crate) fn render_choice(
    frame: &mut String,
    cols: usize,
    rows: usize,
    menu: &ChoiceMenu,
    use_color: bool,
) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let lines = menu.options.len() + 6;
    let mut top = rows.saturating_sub(lines) / 2;
    for _ in 0..top {
        frame.push_str("\x1b[K\r\n");
    }
    let tx = center_x(cols, menu.title.chars().count());
    frame.push_str(&" ".repeat(tx));
    if use_color {
        frame.push_str("\x1b[1;36m");
    }
    frame.push_str(menu.title);
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

    for (idx, (label, is_cur)) in menu.options.iter().enumerate() {
        let is_sel = idx == menu.selected;
        let marker = if is_sel { "> " } else { "  " };
        let cur_tag = if *is_cur { "  (current)" } else { "" };
        let line = format!("{marker}{label}{cur_tag}");
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
    let hint_bot = format!(
        "{}  |  Enter - next  |  Esc - skip setup  |  Q - quit",
        menu.quick_hint
    );
    let bx = center_x(cols, hint_bot.chars().count());
    frame.push_str(&" ".repeat(bx));
    if use_color {
        frame.push_str("\x1b[90m");
    }
    frame.push_str(&hint_bot);
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
