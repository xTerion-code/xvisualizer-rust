use crate::color_menu::ColorMenu;
use crate::layout_menu::LayoutMenu;
use crate::source::PlaybackStream;
use crate::source_menu::SourceMenu;
use crate::theme::Theme;
use crate::theme_menu::ThemeMenu;

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
