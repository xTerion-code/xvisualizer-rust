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

fn row_color_height(row: usize, area_h: usize) -> &'static str {
    let ratio = row as f32 / area_h.max(1) as f32;
    if ratio >= 0.85 {
        "\x1b[31m"
    } else if ratio >= 0.6 {
        "\x1b[33m"
    } else {
        "\x1b[32m"
    }
}

fn freq_color(idx: usize, n_bars: usize) -> &'static str {
    let ratio = idx as f32 / n_bars.max(1) as f32;
    if ratio < 0.2 {
        "\x1b[31m"
    } else if ratio < 0.4 {
        "\x1b[33m"
    } else if ratio < 0.6 {
        "\x1b[32m"
    } else if ratio < 0.8 {
        "\x1b[36m"
    } else {
        "\x1b[35m"
    }
}

pub fn bar_color(
    row: usize,
    area_h: usize,
    bar_idx: usize,
    n_bars: usize,
    mode: ColorMode,
) -> &'static str {
    match mode {
        ColorMode::Height => row_color_height(row, area_h),
        ColorMode::Frequency => freq_color(bar_idx, n_bars),
        ColorMode::Mono => "\x1b[37m",
    }
}
