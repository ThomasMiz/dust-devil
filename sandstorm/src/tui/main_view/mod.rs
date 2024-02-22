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

#[derive(Clone, Copy, PartialEq, Eq)]
enum GraphPrecisionOption {
    OneSecond = 1,
    TwoSeconds = 2,
    FiveSeconds = 5,
    TenSeconds = 10,
    ThirtySeconds = 30,
    OneMinute = 60,
    TwoMinutes = 120,
    FiveMinutes = 300,
    TenMinutes = 600,
}

impl GraphPrecisionOption {
    fn to_str(self) -> &'static str {
        match self {
            Self::OneSecond => "1s",
            Self::TwoSeconds => "2s",
            Self::FiveSeconds => "5s",
            Self::TenSeconds => "10s",
            Self::ThirtySeconds => "30s",
            Self::OneMinute => "1m",
            Self::TwoMinutes => "2m",
            Self::FiveMinutes => "5m",
            Self::TenMinutes => "10m",
        }
    }

    fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::OneSecond),
            1 => Some(Self::TwoSeconds),
            2 => Some(Self::FiveSeconds),
            3 => Some(Self::TenSeconds),
            4 => Some(Self::ThirtySeconds),
            5 => Some(Self::OneMinute),
            6 => Some(Self::TwoMinutes),
            7 => Some(Self::FiveMinutes),
            8 => Some(Self::TenMinutes),
            _ => None,
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        const ALL: &[GraphPrecisionOption] = &[
            GraphPrecisionOption::OneSecond,
            GraphPrecisionOption::TwoSeconds,
            GraphPrecisionOption::FiveSeconds,
            GraphPrecisionOption::TenSeconds,
            GraphPrecisionOption::ThirtySeconds,
            GraphPrecisionOption::OneMinute,
            GraphPrecisionOption::TwoMinutes,
            GraphPrecisionOption::FiveMinutes,
            GraphPrecisionOption::TenMinutes,
        ];

        ALL.iter().copied()
    }

    /// Returns (unit_size_seconds, labels_on_multiples_of: u32, markers_on_multiples_of, print_seconds)
    fn get_values(self) -> (u32, u32, u32, bool) {
        match self {
            Self::OneSecond => (1, 15, 5, true),
            Self::TwoSeconds => (2, 30, 10, true),
            Self::FiveSeconds => (5, 60, 20, false), // TODO: Implement
            Self::TenSeconds => (10, 0, 0, false),
            Self::ThirtySeconds => (30, 0, 0, false),
            Self::OneMinute => (60, 0, 0, false),
            Self::TwoMinutes => (120, 0, 0, false),
            Self::FiveMinutes => (300, 0, 0, false),
            Self::TenMinutes => (600, 0, 0, false),
        }
    }
}

pub struct MainView {
    base: HorizontalSplit<LogBlock, UsageGraph>,
}

impl MainView {
    pub fn new(redraw_notify: Rc<Notify>) -> Self {
        let log_block = LogBlock::new(Rc::clone(&redraw_notify));
        let usage_graph = UsageGraph::new(redraw_notify);

        let base = HorizontalSplit::new(log_block, usage_graph, 0, 1);

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
