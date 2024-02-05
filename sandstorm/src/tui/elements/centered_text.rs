use crossterm::event;
use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::tui::{
    text_wrapper::WrapTextIter,
    ui_element::{HandleEventStatus, UIElement},
};

/// A single line of centered text.
pub struct CenteredTextLine<'a> {
    text: &'a str,
    style: Style,
    text_len: u16,
    current_width: u16,
    text_draw_offset: u16,
}

impl<'a> CenteredTextLine<'a> {
    pub fn new(text: &'a str, style: Style) -> Self {
        Self {
            text_len: text.chars().count().min(u16::MAX as usize) as u16,
            style,
            text,
            current_width: 0,
            text_draw_offset: 0,
        }
    }

    fn resize_if_needed(&mut self, area: Rect) {
        if area.width == self.current_width {
            return;
        }

        self.current_width = area.width;
        self.text_draw_offset = self.current_width.saturating_sub(self.text_len) / 2
    }
}

impl<'a> UIElement for CenteredTextLine<'a> {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.resize_if_needed(area);

        let width = self.text_len.min(self.current_width) as usize;
        buf.set_stringn(area.x + self.text_draw_offset, area.y, self.text, width, self.style);
    }

    fn handle_event(&mut self, _event: &event::Event, _is_focused: bool) -> HandleEventStatus {
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        false
    }

    fn focus_lost(&mut self) {}
}

/// Multiple automatically word-wrapping lines of centered text.
pub struct CenteredText<'a> {
    text: &'a str,
    style: Style,
    lines: Vec<InnerLine<'a>>,
    current_width: u16,
}

struct InnerLine<'a> {
    text: &'a str,
    text_draw_width: u16,
    text_draw_offset: u16,
}

impl<'a> CenteredText<'a> {
    pub fn new(text: &'a str, style: Style) -> Self {
        Self {
            text,
            style,
            lines: Vec::new(),
            current_width: 0,
        }
    }

    pub fn lines_len(&self) -> u16 {
        self.lines.len() as u16
    }

    pub fn resize_if_needed(&mut self, width: u16) {
        if self.current_width == width {
            return;
        }

        self.current_width = width;
        self.lines.clear();

        for text in WrapTextIter::new(self.text, self.current_width as usize) {
            let text_len = text.chars().count().min(u16::MAX as usize) as u16;
            let text_draw_offset = self.current_width.saturating_sub(text_len) / 2;
            self.lines.push(InnerLine {
                text,
                text_draw_width: text_len.min(self.current_width),
                text_draw_offset,
            });
        }
    }
}

impl<'a> UIElement for CenteredText<'a> {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.resize_if_needed(area.width);

        for (i, line) in self.lines.iter_mut().take(area.height as usize).enumerate() {
            let draw_x = area.x + line.text_draw_offset;
            buf.set_stringn(draw_x, area.y + i as u16, line.text, line.text_draw_width as usize, self.style);
        }
    }

    fn handle_event(&mut self, _event: &event::Event, _is_focused: bool) -> HandleEventStatus {
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        false
    }

    fn focus_lost(&mut self) {}
}
