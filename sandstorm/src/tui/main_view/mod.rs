use std::rc::Rc;

use crossterm::event;
use dust_devil_core::logging;
use ratatui::{layout::Rect, Frame};
use tokio::sync::Notify;

use self::log_block::LogBlock;

use super::ui_element::{HandleEventStatus, UIElement};

mod colored_logs;
mod log_block;
mod usage_graph;

pub struct MainView {
    log_block: LogBlock,
}

impl MainView {
    pub fn new(redraw_notify: Rc<Notify>) -> Self {
        let log_block = LogBlock::new(redraw_notify);

        Self { log_block }
    }

    pub fn new_stream_event(&mut self, event: logging::Event) {
        self.log_block.new_stream_event(event);
    }
}

impl UIElement for MainView {
    fn resize(&mut self, area: Rect) {
        self.log_block.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.log_block.render(area, frame);
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
