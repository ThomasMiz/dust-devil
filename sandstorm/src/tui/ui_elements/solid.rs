use std::io::{Error, Write};

use crossterm::{
    style::{ContentStyle, Print, SetStyle},
    QueueableCommand,
};

use crate::tui::types::{HorizontalLine, Rectangle};

use super::{utils::ensure_cursor_at_start, UIElement};

pub struct Solid {
    area: Rectangle,
    fill_char: &'static str,
    style: ContentStyle,
}

impl Solid {
    pub fn new(area: Rectangle, fill_char: &'static str, style: ContentStyle) -> Self {
        Self { area, fill_char, style }
    }
}

impl UIElement for Solid {
    fn area(&self) -> Rectangle {
        self.area
    }

    fn draw_line<O: Write>(
        &mut self,
        out: &mut O,
        area: HorizontalLine,
        mut is_cursor_at_start: bool,
        force_redraw: bool,
    ) -> Result<bool, Error> {
        if !force_redraw {
            return Ok(false);
        }

        ensure_cursor_at_start(&mut is_cursor_at_start, out, area.left(), area.y)?;
        out.queue(SetStyle(self.style))?;
        for _ in 0..area.width() {
            out.queue(Print(self.fill_char))?;
        }

        Ok(true)
    }

    fn resize(&mut self, area: Rectangle) {
        self.area = area;
    }
}
