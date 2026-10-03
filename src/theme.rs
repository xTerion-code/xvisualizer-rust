#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    #[default]
    Height,
    Frequency,
    Mono,
}

impl ColorMode {
    pub fn next(self) -> Self {
        match self {
            ColorMode::Height => ColorMode::Frequency,
            ColorMode::Frequency => ColorMode::Mono,
            ColorMode::Mono => ColorMode::Height,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ColorMode::Height => "Height",
            ColorMode::Frequency => "Frequency",
            ColorMode::Mono => "Mono",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Classic,
    Solid,
}

impl Theme {
    pub fn all() -> [Theme; 2] {
        [Theme::Classic, Theme::Solid]
    }

    pub fn index(self) -> usize {
        match self {
            Theme::Classic => 0,
            Theme::Solid => 1,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Theme::Classic => "Classic",
            Theme::Solid => "Solid",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Theme::Classic => "thin bars with gaps",
            Theme::Solid => "wide bars without gaps",
        }
    }

    pub fn dims(self) -> (usize, usize) {
        match self {
            Theme::Classic => (1, 1),
            Theme::Solid => (2, 0),
        }
    }

    pub fn from_str(s: &str) -> Option<Theme> {
        match s.to_lowercase().as_str() {
            "classic" | "1" => Some(Theme::Classic),
            "solid" | "bold" | "fat" | "2" => Some(Theme::Solid),
            _ => None,
        }
    }

    pub fn fit_bar_count(self, cols: usize, max: usize) -> usize {
        let (bar_w, gap_w) = self.dims();
        let cell = (bar_w + gap_w).max(1);
        let mut n = ((cols.saturating_sub(4) + gap_w) / cell).clamp(1, max);
        // Shrink until the bars actually fit; guards very narrow windows.
        while n > 1 && (n * bar_w + n.saturating_sub(1) * gap_w) > cols {
            n -= 1;
        }
        n
    }
}
