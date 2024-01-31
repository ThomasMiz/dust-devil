use std::{collections::VecDeque, fmt::Write, rc::Rc};

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use dust_devil_core::logging;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    text::Line,
    widgets::{Block, BorderType, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};
use time::{OffsetDateTime, UtcOffset};
use tokio::sync::Notify;

use super::ui_element::{HandleEventStatus, PassFocusDirection, UIElement};

const MAXIMUM_EVENT_HISTORY_LENGTH: usize = 0x10000;
const INITIAL_EVENT_HISTORY_CAPACITY: usize = 0x1000;
const LOG_LINE_MIN_LENGTH: u16 = 20;

const SELECTED_EVENT_BACKGROUND_COLOR: Color = Color::DarkGray;

/// The log block can be scrolled with the arrow keys. Without shift, the arrow keys scroll by one
/// event. With shift, they scroll by this amount.
const KEY_SHIFT_SCROLL_AMOUNT: usize = 5;

pub struct LogBlock {
    redraw_notify: Rc<Notify>,
    current_area: Rect,
    event_history: VecDeque<(logging::Event, usize)>,
    lines: VecDeque<(Line<'static>, usize)>,
    utc_offset: UtcOffset,
    tmp_string: String,
    tmp_vec: Vec<Line<'static>>,
    selected_event_id: Option<usize>,
}

impl LogBlock {
    pub fn new(redraw_notify: Rc<Notify>) -> LogBlock {
        Self {
            redraw_notify,
            current_area: Rect::default(),
            event_history: VecDeque::with_capacity(INITIAL_EVENT_HISTORY_CAPACITY),
            lines: VecDeque::new(),
            utc_offset: UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC),
            tmp_string: String::new(),
            tmp_vec: Vec::new(),
            selected_event_id: None,
        }
    }

    pub fn new_stream_event(&mut self, event: logging::Event) {
        if self.event_history.len() >= MAXIMUM_EVENT_HISTORY_LENGTH {
            if let Some((_oldest_event, oldest_event_id)) = self.event_history.pop_front() {
                while self.lines.front().is_some_and(|(_, id)| *id == oldest_event_id) {
                    self.lines.pop_front();
                }
            }

            if let Some(selected_event_id) = self.selected_event_id {
                if let Some((_oldest_event, oldest_event_id)) = self.event_history.front() {
                    if selected_event_id < *oldest_event_id {
                        self.selected_event_id = Some(*oldest_event_id);
                    }
                }
            }
        }

        let event_id = self.event_history.back().map(|(_, id)| id + 1).unwrap_or(0);
        self.event_history.push_back((event, event_id));

        self.redraw_notify.notify_one();
    }

    fn resize_if_needed(&mut self, new_area: Rect) {
        let previous_area = self.current_area;
        self.current_area = new_area;

        if previous_area.width == new_area.width && previous_area.height == new_area.height {
            return;
        }

        self.lines.clear();
    }

    fn get_focus_position(&self) -> (u16, u16) {
        (
            self.current_area.x + self.current_area.width / 2,
            self.current_area.y + self.current_area.height / 2,
        )
    }

    fn handle_mouse_event(&mut self, _mouse_event: &MouseEvent, _is_focused: bool) -> HandleEventStatus {
        // TODO: Implement

        HandleEventStatus::Unhandled
    }

    fn handle_key_event(&mut self, key_event: &KeyEvent, is_focused: bool) -> HandleEventStatus {
        if !is_focused || key_event.kind == KeyEventKind::Release {
            return HandleEventStatus::Unhandled;
        }

        if key_event.modifiers.contains(KeyModifiers::CONTROL) {
            match key_event.code {
                KeyCode::Up => return HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
                KeyCode::Down => return HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
                // Note: Left and Right keys already pass focus on the match below this one.
                _ => {}
            }
        }

        let previous_selected_event_id = self.selected_event_id;

        let return_value = match key_event.code {
            KeyCode::Up => {
                let pass_focus = match &mut self.selected_event_id {
                    None => {
                        self.selected_event_id = self.event_history.back().map(|(_newest_event, newest_event_id)| *newest_event_id);
                        self.selected_event_id.is_none()
                    }
                    Some(selected_event_id) => {
                        if let Some((_oldest_event, oldest_event_id)) = self.event_history.front() {
                            if *selected_event_id == *oldest_event_id {
                                true
                            } else {
                                let scroll_amount = match key_event.modifiers {
                                    m if m.contains(KeyModifiers::SHIFT) => KEY_SHIFT_SCROLL_AMOUNT,
                                    _ => 1,
                                };

                                *selected_event_id = selected_event_id.saturating_sub(scroll_amount).max(*oldest_event_id);
                                false
                            }
                        } else {
                            true
                        }
                    }
                };

                match pass_focus {
                    true => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
                    false => HandleEventStatus::Handled,
                }
            }
            KeyCode::Down => {
                let pass_focus = match &mut self.selected_event_id {
                    None => {
                        self.selected_event_id = self.event_history.back().map(|(_newest_event, newest_event_id)| *newest_event_id);
                        self.selected_event_id.is_none()
                    }
                    Some(selected_event_id) => {
                        if let Some((_newest_event, newest_event_id)) = self.event_history.back() {
                            if *selected_event_id == *newest_event_id {
                                true
                            } else {
                                let scroll_amount = match key_event.modifiers {
                                    m if m.contains(KeyModifiers::SHIFT) => KEY_SHIFT_SCROLL_AMOUNT,
                                    _ => 1,
                                };

                                *selected_event_id = (*selected_event_id + scroll_amount).min(*newest_event_id);
                                false
                            }
                        } else {
                            true
                        }
                    }
                };

                match pass_focus {
                    true => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
                    false => HandleEventStatus::Handled,
                }
            }
            KeyCode::PageUp => {
                if !is_focused {
                    return HandleEventStatus::Unhandled;
                }

                // TODO: Implement

                HandleEventStatus::Handled
            }
            KeyCode::PageDown => {
                if !is_focused {
                    return HandleEventStatus::Unhandled;
                }

                // TODO: Implement

                HandleEventStatus::Handled
            }
            KeyCode::Home => {
                if !is_focused {
                    return HandleEventStatus::Unhandled;
                }

                // TODO: Implement

                HandleEventStatus::Handled
            }
            KeyCode::End => {
                if !is_focused {
                    return HandleEventStatus::Unhandled;
                }

                // TODO: Implement

                HandleEventStatus::Handled
            }
            KeyCode::Left => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Left),
            KeyCode::Right => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Right),
            KeyCode::Tab => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Forward),
            KeyCode::Esc => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            _ => HandleEventStatus::Unhandled,
        };

        if self.selected_event_id != previous_selected_event_id {
            self.redraw_notify.notify_one();
        }

        return_value
    }

    /// Ensures `self.lines` contains the lines for the event identified by the `center_event_id`
    /// id, as well as at least `needed_line_count` lines around it as to display them. Returns an
    /// iterator with the lines to render (and their corresponding event IDs).
    ///
    /// The event by the id `center_event_id` is assumed to be an existing event id (currently
    /// exists in `self.event_history`).
    ///
    /// ## Dear reader;
    /// This function took a whole day to write. From morning to shut-eye. Whole day of doing absolutely
    /// nothing else but sitting on the computer. Staring at the screen. Thinking about how to tacke the
    /// problem. Rewriting it because "nah that's not elegant". Typing and typing. And debugging to find
    /// that I was simply missing a "-1" somewhere.
    ///
    /// And that's not because it's particularly long, or inefficient, or hacky, or convoluted. On the
    /// contrary; I think it's elegant, performant, efficient, and the exact solution to the problem at
    /// hand. However, it's also quite complex and very much non-trivial.
    ///
    /// Even though I haven't commented most of this codebase, if I didn't comment this function at all
    /// then soon enough it would only be understood by the One True God, השם, HaShem.
    ///
    /// As I write this, at 23:57, I mark on this very file a grave cautionary warning; _don't read this
    /// too hard if you value your mental sanity_. My comments will well serve adventurous explorers,
    /// but not all may make it through the journey.
    fn process_lines_get_iter(
        &mut self,
        center_event_id: usize,
        needed_line_count: usize,
    ) -> Option<impl Iterator<Item = &mut (Line<'static>, usize)>> {
        if self.event_history.is_empty() || needed_line_count == 0 {
            return None;
        }

        let wrap_width = self.current_area.width.max(LOG_LINE_MIN_LENGTH + 2) - 2;

        let oldest_event_id = self.event_history.front().unwrap().1;
        let center_event_index_in_history = center_event_id - oldest_event_id;

        // We start by looking for where in `self.lines` the event with center_event_id is (or is not).
        // This isn't an expensive operation; `self.lines` only carries the lines needed to display the
        // events visible in the viewport for a frame, which is a small amount.
        let maybe_center_start_line_i = self.lines.binary_search_by_key(&center_event_id, |(_, id)| *id);

        // Calculate the event's index in `self.lines` and the amount of lines it has.
        let (center_start_line_i, center_line_count) = match maybe_center_start_line_i {
            Ok(mut index) => {
                // The event is already in `self.lines`, but the entry that the binary search found
                // might not be the first line (an event might be multiple lines), so let's backtrack
                // until we found the first line of the event.
                while index != 0 && self.lines[index - 1].1 == center_event_id {
                    index -= 1;
                }

                // Now let's count how many lines the event has. Events have at most a few lines, so
                // no need for complex searching, a simple linear walk will do.
                let mut line_count = 1;
                while let Some((_, id)) = self.lines.get(index + line_count) {
                    if *id != center_event_id {
                        break;
                    }

                    line_count += 1;
                }

                (index, line_count)
            }
            Err(_) => {
                // The event is not in `self.lines`. We clear the lines deque and add the event alone.
                let selected_event = &self.event_history[center_event_id - self.event_history.front().unwrap().1].0;
                self.lines.clear();
                log_event_to_lines(self.utc_offset, selected_event, wrap_width, &mut self.tmp_string, |line| {
                    self.lines.push_back((line, center_event_id));
                });

                (0, self.lines.len())
            }
        };

        let additional_line_count = needed_line_count.saturating_sub(center_line_count) as i32;

        let mut remaining_older_lines_count = additional_line_count / 2;
        let mut older_events_history_current_i = center_event_index_in_history;
        let mut lines_older_event_current_i = center_start_line_i;

        let mut remaining_newer_lines_count = (additional_line_count + 1) / 2;
        let mut newer_events_history_next_i = center_event_index_in_history + 1;
        let mut lines_newer_event_next_i = center_start_line_i + 1;
        while lines_newer_event_next_i != self.lines.len() && self.lines[lines_newer_event_next_i].1 == center_event_id {
            lines_newer_event_next_i += 1;
        }

        // Turn events from `self.event_history` into lines and add them to `self.lines`
        // progressively, side by side, until there are enough lines on each side or there aren't
        // any more events. Events might already have been turned into lines before, and thus might
        // already be in `self.lines`, in which case they'll be reused.
        while (remaining_older_lines_count > 0 || remaining_newer_lines_count > 0)
            && (older_events_history_current_i > 0 || newer_events_history_next_i < self.event_history.len())
        {
            // Check on which side we need more lines, we'll add lines on the side that needs more.
            if remaining_newer_lines_count < remaining_older_lines_count {
                // We're adding lines on the front (older).
                if older_events_history_current_i == 0 {
                    // There are no more older events! Pass over the remaining to the newer side, so we add
                    // more lines with newer events to compensate and get the same total amount of lines.
                    remaining_newer_lines_count += remaining_older_lines_count;
                    remaining_older_lines_count = 0;
                } else {
                    // There is another older event. If it's already parsed into lines into `self.lines`, we
                    // count those lines and skip over them. Otherwise, we convert the event into lines and
                    // add them into `self.lines`.
                    older_events_history_current_i -= 1;
                    if lines_older_event_current_i == 0 {
                        // Parse the next older event into lines and push them to the front of `self.lines`.
                        let (event, id) = &self.event_history[older_events_history_current_i];

                        // The lines need to be reversed; we have a temporary vector to help with this.
                        self.tmp_vec.clear();
                        log_event_to_lines(self.utc_offset, event, wrap_width, &mut self.tmp_string, |line| {
                            self.tmp_vec.push(line);
                        });

                        for line in self.tmp_vec.drain(..).rev() {
                            self.lines.push_front((line, *id));
                            remaining_older_lines_count -= 1;
                            lines_newer_event_next_i += 1;
                            // ^^ Pushing elements to the front (lower indices) also pushes the back (higher indices)!
                        }
                    } else {
                        // Count and skip over the pre-existing parsed lines of this event.
                        lines_older_event_current_i -= 1;
                        remaining_older_lines_count -= 1;
                        let skipping_event_id = self.lines[lines_older_event_current_i].1;
                        while lines_older_event_current_i != 0 && self.lines[lines_older_event_current_i - 1].1 == skipping_event_id {
                            lines_older_event_current_i -= 1;
                            remaining_older_lines_count -= 1;
                        }
                    }
                }
            } else {
                // We're adding lines on the back (newer).
                if newer_events_history_next_i == self.event_history.len() {
                    // There are no more newer events! Pass over the remaining to the older side, so we add
                    // more lines with older events to compensate and get the same total amount of lines.
                    remaining_older_lines_count += remaining_newer_lines_count;
                    remaining_newer_lines_count = 0;
                } else {
                    // There is another newer event. If it's already parsed into lines into `self.lines`, we
                    // count those lines and skip over them. Otherwise, we convert the events into lines and
                    // add them into `self.lines`.
                    if lines_newer_event_next_i == self.lines.len() {
                        // Parse the next newer event into lines and push them to the back of `self.lines`.
                        let (event, id) = &self.event_history[newer_events_history_next_i];
                        log_event_to_lines(self.utc_offset, event, wrap_width, &mut self.tmp_string, |line| {
                            self.lines.push_back((line, *id));
                            remaining_newer_lines_count -= 1;
                        });
                        lines_newer_event_next_i = self.lines.len();
                    } else {
                        // Count and skip over the pre-existing parsed lines of this event.
                        let skipping_event_id = self.lines[lines_newer_event_next_i].1;
                        lines_newer_event_next_i += 1;
                        remaining_newer_lines_count -= 1;
                        while lines_newer_event_next_i != self.lines.len() && self.lines[lines_newer_event_next_i].1 == skipping_event_id {
                            lines_newer_event_next_i += 1;
                            remaining_newer_lines_count -= 1;
                        }
                    }
                    newer_events_history_next_i += 1;
                }
            }
        }

        // Remove unneeded events from the deque. We remove the back (higher indices) before the
        // front (lower indices) because removing elements from the lower indices will alter the
        // indices of the higher elements. (If you don't like the lower indices being the front and
        // the higher indices being the back, yeah... I get you. But that's VecDeque terminology).
        for _ in lines_newer_event_next_i..self.lines.len() {
            self.lines.pop_back();
        }
        for _ in 0..lines_older_event_current_i {
            self.lines.pop_front();
            lines_newer_event_next_i -= 1;
        }

        // Return the lines to draw as an iterator. The iterator must only cover the parts of the
        // lines deque that should be rendered. Fortunately, there's an easy way we can find that
        // using leftover variables from the previous calculation. The "remaining" counters might
        // be negative, if more lines were found/added than exactly necessary (what can happen if,
        // for example, we need one more line but the next event uses two). Thankfully this is just
        // what we need! If they are negative, that's how many leftover lines we have on each side.
        let (s1, s2) = self.lines.as_mut_slices();
        let mut iter = s1.iter_mut().chain(s2);
        for _ in 0..(-remaining_older_lines_count) {
            iter.next();
        }
        for _ in 0..(-remaining_newer_lines_count) {
            iter.next_back();
        }

        // Still, in some edge cases (an event with more lines than needed_line_count) we might end
        // up with more lines than we should. We can simply limit the iterator to fix this.
        Some(iter.take(needed_line_count))
    }
}

impl UIElement for LogBlock {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.resize_if_needed(area);

        let block = Block::new().border_type(BorderType::Plain).borders(Borders::ALL).title("─Logs");
        let logs_area = block.inner(area);
        block.render(area, buf);

        let selected_event_id = self.selected_event_id;
        let oldest_event_id = self.event_history.front().map(|(_, id)| *id).unwrap_or(0);
        let scroll_bar_position = selected_event_id.map(|id| id - oldest_event_id);
        let mut scrollbar_state = ScrollbarState::new(self.event_history.len())
            .viewport_content_length(logs_area.height as usize)
            .position(scroll_bar_position.unwrap_or(self.event_history.len().saturating_sub(1)));

        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .render(area, buf, &mut scrollbar_state);

        let center_event_id = selected_event_id.unwrap_or(self.event_history.back().map(|(_, id)| *id).unwrap_or(0));
        let lines_iter = self.process_lines_get_iter(center_event_id, logs_area.height as usize);

        let mut y = logs_area.top();
        if let Some(iter) = lines_iter {
            for (line, event_id) in iter {
                for ele in line.spans.iter_mut() {
                    ele.style.bg = match Some(*event_id) == selected_event_id {
                        true => Some(SELECTED_EVENT_BACKGROUND_COLOR),
                        false => None,
                    };
                }
                buf.set_line(logs_area.left(), y, line, logs_area.width);
                y += 1;
            }
        }
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        match event {
            event::Event::Mouse(mouse_event) => self.handle_mouse_event(mouse_event, is_focused),
            event::Event::Key(key_event) => self.handle_key_event(key_event, is_focused),
            _ => HandleEventStatus::Unhandled,
        }
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        if let Some((_newest_event, newest_event_id)) = self.event_history.back() {
            self.selected_event_id = Some(*newest_event_id);
            self.redraw_notify.notify_one();
            true
        } else {
            false
        }
    }

    fn focus_lost(&mut self) {
        if self.selected_event_id.is_some() {
            self.selected_event_id = None;
            self.redraw_notify.notify_one();
        }
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
