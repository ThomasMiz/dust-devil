use std::rc::Rc;

use crossterm::event;
use dust_devil_core::logging;
use ratatui::{layout::Rect, Frame};
use tokio::sync::Notify;

use self::{log_block::LogBlock, usage_graph::UsageGraph};

use super::{
    elements::horizontal_split::HorizontalSplit,
    ui_element::{HandleEventStatus, UIElement},
};

mod colored_logs;
mod log_block;
mod usage_graph;
mod usage_tracker;

pub struct MainView {
    base: HorizontalSplit<LogBlock, UsageGraph>,
}

impl MainView {
    pub fn new(redraw_notify: Rc<Notify>) -> Self {
        let log_block = LogBlock::new(Rc::clone(&redraw_notify));
        let usage_graph = UsageGraph::new(redraw_notify);

        let base = HorizontalSplit::new(log_block, usage_graph, 0, 0);

        Self { base }
    }

    pub fn new_stream_event(&mut self, event: logging::Event) {
        let usage_graph = &mut self.base.right;
        match &event.data {
            logging::EventData::ClientBytesSent(_, sent) => {
                usage_graph.record_usage(event.timestamp, *sent, 0);
            }
            logging::EventData::ClientBytesReceived(_, received) => {
                usage_graph.record_usage(event.timestamp, 0, *received);
            }
            _ => {}
        }

        let log_block = &mut self.base.left;
        log_block.new_stream_event(event);
    }
}

impl UIElement for MainView {
    fn resize(&mut self, area: Rect) {
        self.base.left_width = area.width / 2;
        self.base.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.base.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.base.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.base.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.base.focus_lost();
    }
}
