use crossterm::event;
use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::tui::ui_element::{HandleEventStatus, UIElement};

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
