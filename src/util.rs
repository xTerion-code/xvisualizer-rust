const MIN: f32 = 0.2;
const MAX: f32 = 8.0;
const STEP: f32 = 1.25;

pub struct Gain {
    value: f32,
}

impl Gain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn up(&mut self) {
        self.value = (self.value * STEP).min(MAX);
    }

    pub fn down(&mut self) {
        self.value = (self.value / STEP).max(MIN);
    }

    pub fn reset(&mut self) {
        self.value = 1.0;
    }
}

impl Default for Gain {
    fn default() -> Self {
        Self { value: 1.0 }
    }
}

pub fn truncate_chars(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        return s;
    }
    let idx = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    &s[..idx]
}

pub fn center_x(cols: usize, width: usize) -> usize {
    cols.saturating_sub(width) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_clamp_at_bounds() {
        let mut g = Gain::new();
        for _ in 0..20 {
            g.up();
        }
        assert_eq!(g.value(), MAX);
        for _ in 0..40 {
            g.down();
        }
        assert_eq!(g.value(), MIN);
        g.reset();
        assert_eq!(g.value(), 1.0);
    }

    #[test]
    fn short_strings_pass_through() {
        assert_eq!(truncate_chars("abc", 60), "abc");
    }

    #[test]
    fn long_strings_cut_on_char_boundary() {
        assert_eq!(truncate_chars("abcdef", 4), "abcd");
        // Combining mark is its own char; cut keeps whole chars only.
        assert_eq!(truncate_chars("a\u{301}bc", 1), "a");
        assert_eq!(truncate_chars("a\u{301}bc", 2), "a\u{301}");
    }

    #[test]
    fn centering_clamps_on_overflow() {
        assert_eq!(center_x(80, 10), 35);
        assert_eq!(center_x(5, 10), 0);
    }
}
