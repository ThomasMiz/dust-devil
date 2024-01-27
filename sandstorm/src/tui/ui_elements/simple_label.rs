use std::io::{Error, Write};

use crossterm::{
    style::{ContentStyle, Print, SetStyle},
    QueueableCommand,
};

use crate::tui::types::{HorizontalLine, Rectangle};

use super::{utils::ensure_cursor_at_start, UIElement};

/// A Simple, single-line label that left-aligns the text and provides no wrapping. If the text is
/// not large enough to cover the label, the leftover space is filled with space characters. If the
/// text is too large to fit in the label, it is cut off abruptly.
pub struct SimpleLabel {
    area: HorizontalLine,
    text: String,
    text_len: u16,
    text_style: ContentStyle,
}

impl SimpleLabel {
    pub fn new(area: HorizontalLine, text: String, text_style: ContentStyle) -> Self {
        Self {
            area,
            text_len: text.chars().count().min(u16::MAX as usize) as u16,
            text,
            text_style,
        }
    }

    pub fn area_as_line(&self) -> HorizontalLine {
        self.area
    }

    pub fn text_len(&self) -> u16 {
        self.text_len
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn modify_text<F: FnOnce(&mut String)>(&mut self, f: F) -> &str {
        f(&mut self.text);
        self.text_len = self.text.chars().count().min(u16::MAX as usize) as u16;
        &self.text
    }
}

impl UIElement for SimpleLabel {
    fn area(&self) -> Rectangle {
        self.area.into()
    }

    fn draw_line<O: Write>(
        &mut self,
        out: &mut O,
        area: HorizontalLine,
        mut is_cursor_at_start: bool,
        force_redraw: bool,
    ) -> Result<bool, Error> {
        if area.y == self.area.y && force_redraw {
            ensure_cursor_at_start(&mut is_cursor_at_start, out, area.left(), area.y)?;
            out.queue(SetStyle(self.text_style))?;

            let text_start_x = area.left();
            let text_end_x_exclusive = (area.right() + 1).min(self.area.left() + self.text_len);
            let text_draw_width = text_end_x_exclusive.max(text_start_x) - text_start_x;

            if text_draw_width != 0 {
                let mut chars_iter = self.text.char_indices().skip((text_start_x - self.area.left()) as usize);
                for _ in 0..text_draw_width {
                    let (index_at, c) = chars_iter.next().unwrap();
                    out.queue(Print(&self.text[index_at..(index_at + c.len_utf8())]))?;
                }
            }

            for _ in 0..(area.width() - text_draw_width) {
                out.queue(Print(" "))?;
            }
        } else {
            is_cursor_at_start = false;
        }

        Ok(is_cursor_at_start)
    }

    fn resize(&mut self, area: Rectangle) {
        self.area = area.top_as_line();
    }
}
