use std::rc::Rc;

use crossterm::event;
use dust_devil_core::logging;
use ratatui::{buffer::Buffer, layout::Rect};
use tokio::sync::Notify;

use super::{
    log_block::LogBlock,
    ui_element::{HandleEventStatus, UIElement},
};

// Note: right now this UIElement is just a wrapper around a log block, but soon enough more
// things will be added to it.

pub struct BottomArea {
    log_block: LogBlock,
}

impl BottomArea {
    pub fn new(redraw_notify: Rc<Notify>) -> Self {
        let log_block = LogBlock::new(redraw_notify);

        Self { log_block }
    }

    pub fn new_stream_event(&mut self, event: logging::Event) {
        self.log_block.new_stream_event(event);
    }
}

impl UIElement for BottomArea {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.log_block.render(area, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.log_block.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.log_block.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.log_block.focus_lost();
    }
}
