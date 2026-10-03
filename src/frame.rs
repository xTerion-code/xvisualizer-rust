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
