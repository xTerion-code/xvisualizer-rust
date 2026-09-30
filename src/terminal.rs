use std::io::{self, Write};

use crossterm::{cursor, execute, terminal};

pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = terminal::disable_raw_mode();
        let _ = execute!(out, cursor::Show);
        let _ = execute!(out, terminal::LeaveAlternateScreen);
    }
}

pub fn enter(out: &mut impl Write) -> io::Result<()> {
    execute!(out, terminal::EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    execute!(out, cursor::Hide)?;
    Ok(())
}

// Raw mode also disables `\n` -> `\r\n` translation (see render.rs).
pub fn size() -> (usize, usize) {
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    (cols as usize, rows as usize)
}
