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
    fn short_strings_pass_through() {
        assert_eq!(truncate_chars("abc", 60), "abc");
    }

    #[test]
    fn long_strings_cut_on_char_boundary() {
        assert_eq!(truncate_chars("abcdef", 4), "abcd");
        // Combining mark is its own char; cut keeps whole chars only.
        assert_eq!(truncate_chars("а\u{301}бв", 1), "а");
        assert_eq!(truncate_chars("а\u{301}бв", 2), "а\u{301}");
    }

    #[test]
    fn centering_clamps_on_overflow() {
        assert_eq!(center_x(80, 10), 35);
        assert_eq!(center_x(5, 10), 0);
    }
}
