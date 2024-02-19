use std::ops::Deref;

use crossterm::event;
use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    Frame,
};

use crate::tui::{
    text_wrapper::{StaticString, WrapTextIter},
    ui_element::{AutosizeUIElement, HandleEventStatus, UIElement},
};

/// A single-line text.
pub struct TextLine {
    text: StaticString,
    style: Style,
    alignment: Alignment,
    text_len: u16,
    current_width: u16,
    text_draw_offset: u16,
}

impl TextLine {
    pub fn new(text: StaticString, style: Style, alignment: Alignment) -> Self {
        let text_len = text.chars().count().min(u16::MAX as usize) as u16;
        Self {
            text,
            style,
            alignment,
            text_len,
            current_width: 0,
            text_draw_offset: 0,
        }
    }
}

impl UIElement for TextLine {
    fn resize(&mut self, area: Rect) {
        if area.width == self.current_width {
            return;
        }

        self.current_width = area.width;
        self.text_draw_offset = match self.alignment {
            Alignment::Left => 0,
            Alignment::Center => self.current_width.saturating_sub(self.text_len) / 2,
            Alignment::Right => self.current_width.saturating_sub(self.text_len),
        };
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let width = self.text_len.min(self.current_width) as usize;
        let buf = frame.buffer_mut();
        buf.set_stringn(area.x + self.text_draw_offset, area.y, self.text.deref(), width, self.style);
    }

    fn handle_event(&mut self, _event: &event::Event, _is_focused: bool) -> HandleEventStatus {
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        false
    }

    fn focus_lost(&mut self) {}
}

impl AutosizeUIElement for TextLine {
    fn begin_resize(&mut self, width: u16, _height: u16) -> (u16, u16) {
        (width.min(self.text_len), 1)
    }
}

/// Multiple automatically word-wrapping lines of centered text.
pub struct Text {
    text: StaticString,
    style: Style,
    alignment: Alignment,
    lines: Vec<InnerLine>,
    current_width: u16,
}

struct InnerLine {
    text_index: usize,
    text_len_bytes: u32,
    text_draw_width: u16,
    text_draw_offset: u16,
}

impl Text {
    pub fn new(text: StaticString, style: Style, alignment: Alignment) -> Self {
        Self {
            text,
            style,
            alignment,
            lines: Vec::new(),
            current_width: 0,
        }
    }

    pub fn lines_len(&self) -> u16 {
        self.lines.len() as u16
    }

    pub fn modify_text<F: FnOnce(&mut StaticString)>(&mut self, f: F) {
        f(&mut self.text);
        self.resize_no_check(self.current_width);
    }

    pub fn style(&self) -> Style {
        self.style
    }

    fn resize_no_check(&mut self, width: u16) {
        self.current_width = width;
        self.lines.clear();

        for item in WrapTextIter::new(self.text.deref(), self.current_width as usize) {
            let text_draw_offset = match self.alignment {
                Alignment::Left => 0,
                Alignment::Center => self.current_width.saturating_sub(item.len_chars as u16) / 2,
                Alignment::Right => self.current_width.saturating_sub(item.len_chars as u16),
            };

            self.lines.push(InnerLine {
                text_index: item.index_start,
                text_len_bytes: item.len_bytes as u32,
                text_draw_width: self.current_width.min(item.len_chars as u16),
                text_draw_offset,
            });
        }
    }

    fn resize_with_width(&mut self, width: u16) {
        if self.current_width == width {
            return;
        }

        self.resize_no_check(width);
    }
}

impl UIElement for Text {
    fn resize(&mut self, area: Rect) {
        self.resize_with_width(area.width);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        for (i, line) in self.lines.iter_mut().take(area.height as usize).enumerate() {
            let draw_x = area.x + line.text_draw_offset;
            let string = &self.text[line.text_index..(line.text_index + line.text_len_bytes as usize)];
            let buf = frame.buffer_mut();
            buf.set_stringn(draw_x, area.y + i as u16, string, line.text_draw_width as usize, self.style);
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

impl AutosizeUIElement for Text {
    fn begin_resize(&mut self, width: u16, _height: u16) -> (u16, u16) {
        self.resize_with_width(width);
        let text_width = self.lines.iter().map(|line| line.text_draw_width).max();
        (text_width.unwrap_or(0), self.lines_len())
    }
}
