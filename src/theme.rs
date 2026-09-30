//! Bar-style themes.
//!
//! This module only defines the available themes and their geometry.
//! It knows nothing about audio, input or rendering.

/// Bar style theme.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Classic,
    Solid,
}

impl Theme {
    /// All themes in menu order.
    pub fn all() -> [Theme; 2] {
        [Theme::Classic, Theme::Solid]
    }

    /// Index of this theme in [`Theme::all`].
    pub fn index(self) -> usize {
        match self {
            Theme::Classic => 0,
            Theme::Solid => 1,
        }
    }

    /// Short display name.
    pub fn name(self) -> &'static str {
        match self {
            Theme::Classic => "Classic",
            Theme::Solid => "Solid",
        }
    }

    /// One-line description shown in the theme menu.
    pub fn desc(self) -> &'static str {
        match self {
            Theme::Classic => "thin bars with gaps",
            Theme::Solid => "wide bars without gaps",
        }
    }

    /// (bar width, gap between bars) in terminal cells.
    pub fn dims(self) -> (usize, usize) {
        match self {
            Theme::Classic => (1, 1),
            Theme::Solid => (2, 0),
        }
    }

    /// Parses a `--theme` CLI value.
    pub fn from_str(s: &str) -> Option<Theme> {
        match s.to_lowercase().as_str() {
            "classic" | "1" => Some(Theme::Classic),
            "solid" | "bold" | "fat" | "2" => Some(Theme::Solid),
            _ => None,
        }
    }

    /// How many bars fit into `cols` columns for this theme,
    /// clamped to `max` and guarded against very narrow windows.
    pub fn fit_bar_count(self, cols: usize, max: usize) -> usize {
        let (bar_w, gap_w) = self.dims();
        let cell = (bar_w + gap_w).max(1);
        let mut n = ((cols.saturating_sub(4) + gap_w) / cell).clamp(8, max);
        while n > 8 && (n * bar_w + n.saturating_sub(1) * gap_w) > cols {
            n -= 1;
        }
        n
    }
}
