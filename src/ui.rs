use crate::color::ColorMode;
use crate::source::{CaptureTarget, PlaybackStream, list_playback_streams};
use crate::symmetry::LayoutMode;
use crate::theme::Theme;

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
    pub layout: LayoutMode,
    pub in_help: bool,
}

// Source switching reroutes PulseAudio and restarts capture, so the handler
// only returns a request and `main` applies it.
pub enum UiEvent {
    SelectSystem,
    SelectStream(PlaybackStream),
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
            layout: LayoutMode::default(),
            in_help: false,
        }
    }

    pub(crate) fn open_menu(&mut self) {
        self.selected = self.current.index();
        self.in_source_menu = false;
        self.in_help = false;
        self.in_menu = true;
    }

    pub(crate) fn step(&mut self, dir: i32) {
        let n = Theme::all().len() as i32;
        self.selected = (self.selected as i32 + dir).rem_euclid(n) as usize;
    }

    pub(crate) fn open_source_menu(&mut self) {
        self.apps = list_playback_streams();
        self.source_selected = self.target.menu_index(&self.apps);
        self.in_menu = false;
        self.in_help = false;
        self.in_source_menu = true;
    }

    pub(crate) fn source_step(&mut self, dir: i32) {
        let n = self.apps.len() as i32 + 1;
        self.source_selected = (self.source_selected as i32 + dir).rem_euclid(n) as usize;
    }

    pub(crate) fn refresh_apps(&mut self) {
        self.apps = list_playback_streams();
        let max = self.apps.len();
        self.source_selected = self.source_selected.min(max);
    }

    pub(crate) fn confirm(&mut self) {
        self.current = Theme::all()[self.selected];
        self.in_menu = false;
    }

    pub(crate) fn cancel(&mut self) {
        self.selected = self.current.index();
        self.in_menu = false;
    }

    pub(crate) fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub(crate) fn cycle_color(&mut self) {
        self.color = self.color.next();
    }

    pub(crate) fn toggle_layout(&mut self) {
        self.layout = self.layout.next();
    }

    pub(crate) fn toggle_help(&mut self) {
        self.in_help = !self.in_help;
    }
}
