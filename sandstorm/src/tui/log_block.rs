use std::{collections::VecDeque, rc::Rc};

use crossterm::event;
use dust_devil_core::logging;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    Frame,
};
use time::UtcOffset;
use tokio::sync::Notify;

use super::{
    colored_logs::log_event_to_single_line,
    elements::{
        long_list::{LongList, LongListHandler},
        OnEnterResult,
    },
    text_wrapper::{wrap_lines_by_chars, StaticString},
    ui_element::{HandleEventStatus, UIElement},
};

const MAXIMUM_EVENT_HISTORY_LENGTH: usize = 0x10000;
const INITIAL_EVENT_HISTORY_CAPACITY: usize = 0x1000;
const LOG_LINE_MIN_LENGTH: u16 = 20;

const TITLE: &str = "─Logs";

const SELECTED_EVENT_BACKGROUND_COLOR: Color = Color::DarkGray;
const SELECTED_TIMESTAMP_BACKGROUND_COLOR: Color = Color::Gray;

pub struct LogBlock {
    list: LongList<LogListHandler>,
}

struct LogListHandler {
    event_history: VecDeque<logging::Event>,
    utc_offset: UtcOffset,
    tmp_line_vec: Vec<(StaticString, Style)>,
}

impl LongListHandler for LogListHandler {
    fn get_item_lines<F: FnMut(Line<'static>)>(&mut self, index: usize, wrap_width: u16, f: F) {
        let event = &self.event_history[index];
        log_event_to_single_line(&mut self.tmp_line_vec, self.utc_offset, event);

        let wrap_width = wrap_width.max(LOG_LINE_MIN_LENGTH) as usize;
        wrap_lines_by_chars(wrap_width, self.tmp_line_vec.drain(..), f);
    }

    fn modify_line_to_selected(&mut self, _index: usize, line: &mut Line<'static>, item_line_number: u16) {
        let mut spans_iter = line.spans.iter_mut();

        if item_line_number == 0 {
            if let Some(first_span) = spans_iter.next() {
                first_span.style.bg = Some(SELECTED_TIMESTAMP_BACKGROUND_COLOR);
            }
        }

        for span in spans_iter {
            span.style.bg = Some(SELECTED_EVENT_BACKGROUND_COLOR);
        }
    }

    fn modify_line_to_unselected(&mut self, _index: usize, line: &mut Line<'static>, _item_line_number: u16) {
        for span in line.spans.iter_mut() {
            span.style.bg = None;
        }
    }

    fn on_enter(&mut self, index: usize) -> OnEnterResult {
        OnEnterResult::Unhandled
    }
}

impl LogBlock {
    pub fn new(redraw_notify: Rc<Notify>) -> Self {
        let handler = LogListHandler {
            event_history: VecDeque::with_capacity(INITIAL_EVENT_HISTORY_CAPACITY),
            utc_offset: UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC),
            tmp_line_vec: Vec::with_capacity(10),
        };

        Self {
            list: LongList::new(redraw_notify, TITLE.into(), 0, true, false, handler),
        }
    }

    pub fn new_stream_event(&mut self, event: logging::Event) {
        if self.list.handler.event_history.len() >= MAXIMUM_EVENT_HISTORY_LENGTH {
            self.list.handler.event_history.pop_front();
            self.list.on_item_removed(0);
        }

        self.list.handler.event_history.push_back(event);
        self.list.set_item_count(self.list.handler.event_history.len());
    }
}

impl UIElement for LogBlock {
    fn resize(&mut self, area: Rect) {
        self.list.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.list.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.list.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.list.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.list.focus_lost()
    }
}
