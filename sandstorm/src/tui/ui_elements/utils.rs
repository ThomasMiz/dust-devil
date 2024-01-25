use std::io::{Error, Write};

use crossterm::{cursor::MoveTo, QueueableCommand};

#[inline]
pub fn ensure_cursor_at_start<O: Write>(is_cursor_at_start: &mut bool, out: &mut O, x: u16, y: u16) -> Result<(), Error> {
    if !*is_cursor_at_start {
        out.queue(MoveTo(x, y))?;
        *is_cursor_at_start = true;
    }

    Ok(())
}
