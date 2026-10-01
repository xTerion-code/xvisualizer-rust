use std::io::{self, Write};

use crossterm::{cursor, execute, terminal};

pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

/// Best-effort restore, also used by the panic hook in `main`.
pub fn restore() {
    let mut out = io::stdout();
    let _ = terminal::disable_raw_mode();
    let _ = execute!(out, cursor::Show);
    let _ = execute!(out, terminal::LeaveAlternateScreen);
}

pub fn enter(out: &mut impl Write) -> io::Result<TerminalGuard> {
    execute!(&mut *out, terminal::EnterAlternateScreen)?;
    if let Err(e) = terminal::enable_raw_mode() {
        let _ = execute!(&mut *out, terminal::LeaveAlternateScreen);
        return Err(e);
    }
    if let Err(e) = execute!(&mut *out, cursor::Hide) {
        restore();
        return Err(e);
    }
    Ok(TerminalGuard)
}

// Raw mode also disables `\n` -> `\r\n` translation (see render.rs).
pub fn size() -> (usize, usize) {
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    (cols as usize, rows as usize)
}
