/// Center-symmetric spectrum mapping: bass in the middle, treble at the edges.
/// The analyzer produces `unique` bands (low -> high); the visualizer mirrors
/// them into `unique * 2 - 1` display bars so left and right stay symmetric.
pub fn unique_count(display_fit: usize) -> usize {
    display_fit.div_ceil(2).max(1)
}

pub fn display_count(unique: usize) -> usize {
    unique.saturating_mul(2).saturating_sub(1).max(1)
}

/// Display position `d` (0 = left edge) to spectrum index (0 = bass).
pub fn spectrum_index(display_idx: usize, unique: usize) -> usize {
    if unique <= 1 {
        return 0;
    }
    let center = unique - 1;
    (display_idx as isize - center as isize).unsigned_abs()
}
