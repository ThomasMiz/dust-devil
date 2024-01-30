use std::{collections::VecDeque, fmt::Write, rc::Rc};

use crossterm::event;
use dust_devil_core::logging;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::Line,
    widgets::{Block, BorderType, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};
use time::{OffsetDateTime, UtcOffset};
use tokio::sync::Notify;

use super::ui_element::{HandleEventStatus, UIElement};

const MAXIMUM_EVENT_HISTORY_LENGTH: usize = 1024;
const INITIAL_EVENT_HISTORY_CAPACITY: usize = 512;
const LOG_LINE_MIN_LENGTH: u16 = 20;

pub struct LogBlock {
    redraw_notify: Rc<Notify>,
    current_size: (u16, u16),
    log_history: VecDeque<logging::Event>,
    lines: VecDeque<Line<'static>>,
    utc_offset: UtcOffset,
    tmp_string: String,
}

impl LogBlock {
    pub fn new(redraw_notify: Rc<Notify>) -> LogBlock {
        Self {
            redraw_notify,
            current_size: (0, 0),
            log_history: VecDeque::new(),
            lines: VecDeque::with_capacity(INITIAL_EVENT_HISTORY_CAPACITY),
            utc_offset: UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC),
            tmp_string: String::new(),
        }
    }

    pub fn new_stream_event(&mut self, event: logging::Event) {
        if self.log_history.len() >= MAXIMUM_EVENT_HISTORY_LENGTH {
            self.log_history.pop_front();
        }

        let wrap_width = self.current_size.0.max(LOG_LINE_MIN_LENGTH + 2) - 2;
        log_event_to_lines(self.utc_offset, &event, wrap_width, &mut self.tmp_string, |line| {
            self.lines.push_back(line);
        });

        self.log_history.push_back(event);
        self.redraw_notify.notify_one();
    }

    fn resize_if_needed(&mut self, new_width: u16, new_height: u16) {
        if self.current_size.0 == new_width && self.current_size.1 == new_height {
            return;
        }

        self.current_size = (new_width, new_height);

        let wrap_width = new_width.max(LOG_LINE_MIN_LENGTH + 2) - 2;
        self.lines.clear();
        for event in &self.log_history {
            log_event_to_lines(self.utc_offset, event, wrap_width, &mut self.tmp_string, |line| {
                self.lines.push_back(line);
            })
        }
    }
}

impl UIElement for LogBlock {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.resize_if_needed(area.width, area.height);

        let block = Block::new().border_type(BorderType::Plain).borders(Borders::ALL).title("─Logs");
        let logs_area = block.inner(area);
        block.render(area, buf);

        let mut scrollbar_state = ScrollbarState::new(self.lines.len())
            .viewport_content_length(logs_area.height as usize)
            .position(self.lines.len() / 2);

        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .render(area, buf, &mut scrollbar_state);

        let max_lines_i = self.lines.len().min(logs_area.height as usize);
        for i in 0..max_lines_i as u16 {
            let line = &self.lines[i as usize];
            buf.set_line(logs_area.left(), logs_area.top() + i, line, logs_area.width);
        }
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        // TODO: Implement event handling
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        // TODO: Implement focus handling
        false
    }

    fn focus_lost(&mut self) {
        // TODO: Implement focus handling
    }
}

fn log_event_to_lines<F>(utc_offset: UtcOffset, event: &logging::Event, wrap_width: u16, tmp_string: &mut String, mut f: F)
where
    F: FnMut(Line<'static>),
{
    tmp_string.clear();

    let t = OffsetDateTime::from_unix_timestamp(event.timestamp)
        .map(|t| t.to_offset(utc_offset))
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);

    let _ = write!(
        tmp_string,
        "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}] {}",
        t.year(),
        t.month() as u8,
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
        event.data
    );

    let mut chars = tmp_string.chars();
    let mut s = String::new();
    let mut keep_going = true;
    while keep_going {
        for _ in 0..wrap_width {
            match chars.next() {
                Some(c) => s.push(c),
                None => {
                    keep_going = false;
                    break;
                }
            }
        }

        if !s.is_empty() {
            f(Line::from(s));
            s = String::new();
        }
    }
}
