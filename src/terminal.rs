//! Terminal setup and guaranteed restore (including on panic).
//!
//! This module only manages the terminal mode. It performs no rendering.

use std::io::{self, Write};

use crossterm::{cursor, execute, terminal};

/// Restores the terminal to a normal state when dropped.
pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = terminal::disable_raw_mode();
        let _ = execute!(out, cursor::Show);
        let _ = execute!(out, terminal::LeaveAlternateScreen);
    }
}

/// Enters the alternate screen, enables raw mode and hides the cursor.
///
/// Raw mode is required for arrow-key input. It also disables the
/// terminal's `\n` -> `\r\n` translation, so rendering must use `\r\n`.
pub fn enter(out: &mut impl Write) -> io::Result<()> {
    execute!(out, terminal::EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    execute!(out, cursor::Hide)?;
    Ok(())
}

/// Current terminal size in cells, with a safe fallback.
pub fn size() -> (usize, usize) {
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    (cols as usize, rows as usize)
}
