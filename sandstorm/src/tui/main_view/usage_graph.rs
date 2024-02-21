use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt::Write,
    ops::DerefMut,
    rc::{Rc, Weak},
    time::Duration,
};

use crossterm::event;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    Frame,
};
use time::{OffsetDateTime, UtcOffset};
use tokio::{sync::Notify, task::JoinHandle, time::MissedTickBehavior};

use crate::tui::{
    pretty_print::PrettyByteDisplayer,
    ui_element::{HandleEventStatus, UIElement},
};

use super::usage_tracker::{UsageMeasure, UsageTracker};

pub const MAX_HISTORY_SECONDS: usize = 60 * 60 * 6; // 6 hours

const BLOCK_CHAR: char = '█';
const HALF_BLOCK_CHAR: char = '▄';

const MARKER_ZERO: &str = "0B/s";

const VERTICAL_LABELS_AREA_WIDTH: u16 = 9;
const VERTICAL_CHAR: char = '│';
const HORIZONTAL_CHAR: char = '─';
const CROSS_CHAR: char = '┼';

const LABEL_STYLE: Style = Style::new();
const AXIS_STYLE: Style = Style::new();

const SEND_COLOR: Color = Color::LightGreen;
const RECEIVE_COLOR: Color = Color::LightBlue;

fn pretty_round_max_bytes_value(max: u64) -> u64 {
    match max {
        0 => 0,
        m => m.max(64).checked_next_power_of_two().unwrap_or(m),
    }
}

struct ControllerInner {
    history: UsageTracker,
}

struct Controller {
    inner: RefCell<ControllerInner>,
    redraw_notify: Rc<Notify>,
}

impl Controller {
    fn new(redraw_notify: Rc<Notify>) -> Self {
        let history = UsageTracker::new(MAX_HISTORY_SECONDS);
        let inner = RefCell::new(ControllerInner { history });

        Self { inner, redraw_notify }
    }
}

struct VerticalAxis {
    current_max: u64,
    distance_between_markers: u16,
    marker_labels: Vec<String>,
}

impl VerticalAxis {
    fn new() -> Self {
        Self {
            current_max: 0,
            distance_between_markers: 0,
            marker_labels: Vec::with_capacity(16),
        }
    }

    fn resize(&mut self, height: u16) {
        self.current_max = 0;

        self.distance_between_markers = match height {
            h if h % 3 == 0 => 3,
            h if h % 4 == 0 => 4,
            h if (h - 1) % 3 == 0 => 3,
            h if (h - 1) % 4 == 0 => 4,
            _ => 3,
        };

        let marker_count = ((height - 1) / self.distance_between_markers) as usize;
        if self.marker_labels.len() < marker_count {
            while self.marker_labels.len() < marker_count {
                self.marker_labels.push(String::new());
            }
        } else {
            self.marker_labels.truncate(marker_count);
        }
    }

    fn recalculate_if_needed(&mut self, max: u64) {
        if self.current_max == max {
            return;
        }

        self.current_max = max;
        if max != 0 {
            let marker_count = self.marker_labels.len() as u64;
            for (i, label) in self.marker_labels.iter_mut().enumerate() {
                let bytes = max * (i + 1) as u64 / marker_count;
                label.clear();
                let _ = write!(label, "{}/s", PrettyByteDisplayer(bytes as usize));
            }
        }
    }

    fn render(&mut self, max: u64, area: Rect, buf: &mut Buffer) {
        self.recalculate_if_needed(max);

        let mut y = area.bottom() - 1;
        buf.set_string(area.right() - 3 - MARKER_ZERO.len() as u16, y, MARKER_ZERO, LABEL_STYLE);

        let idx = buf.index_of(area.right() - 2, y);
        buf.content[idx].set_char(HORIZONTAL_CHAR).set_style(AXIS_STYLE);
        buf.content[idx + 1].set_char(CROSS_CHAR).set_style(AXIS_STYLE);

        let x = area.right() - 1;
        if max == 0 {
            for i in 1..=(area.height - 1) {
                buf.get_mut(x, area.bottom() - 1 - i).set_char(VERTICAL_CHAR).set_style(AXIS_STYLE);
            }
        } else {
            for label in &self.marker_labels {
                y -= self.distance_between_markers;
                buf.set_string(area.right() - 2 - label.len() as u16, y, label, LABEL_STYLE);
            }

            for i in 1..=(area.height - 1) {
                let ch = match i % self.distance_between_markers {
                    0 => CROSS_CHAR,
                    _ => VERTICAL_CHAR,
                };

                buf.get_mut(x, area.bottom() - 1 - i).set_char(ch).set_style(AXIS_STYLE);
            }
        }
    }
}

struct HorizontalAxis {
    utc_offset: UtcOffset,
    unit_size_seconds: u32,
    labels_on_multiples_of: u32,
    markers_on_multiples_of: u32,
    print_seconds: bool,
    label_strings: VecDeque<String>,
    latest_label_timestamp: i64,
    string_recycle_bin: Option<String>,
}

fn write_timestamp_to_string(string: &mut String, timestamp: i64, utc_offset: UtcOffset, print_seconds: bool) {
    let t = OffsetDateTime::from_unix_timestamp(timestamp)
        .map(|t| t.to_offset(utc_offset))
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);

    string.clear();
    let _ = write!(string, "{:02}:{:02}", t.hour(), t.minute());
    if print_seconds {
        let _ = write!(string, ":{:02}", t.second());
    }
}

impl HorizontalAxis {
    fn new() -> Self {
        Self {
            utc_offset: UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC),
            unit_size_seconds: 0,
            labels_on_multiples_of: 0,
            markers_on_multiples_of: 0,
            print_seconds: false,
            label_strings: VecDeque::with_capacity(16),
            latest_label_timestamp: 0,
            string_recycle_bin: None,
        }
    }

    fn get_new_string(&mut self) -> String {
        self.string_recycle_bin.take().unwrap_or_default()
    }

    fn recalculate_if_needed(&mut self, unit_size_seconds: u32, latest_timestamp: i64) {
        if self.unit_size_seconds == unit_size_seconds {
            return;
        }

        //self.unit_size_seconds = unit_size_seconds;
        // TODO: Properly set unit_size_seconds and choose the following values based on it
        self.unit_size_seconds = 1;
        self.labels_on_multiples_of = 15;
        self.markers_on_multiples_of = 5;
        self.print_seconds = true;

        let labels_on_multiples_of = self.labels_on_multiples_of as i64;
        let latest_label_timestamp = latest_timestamp / labels_on_multiples_of * labels_on_multiples_of;

        let mut deque_index = self.label_strings.len();
        if self.label_strings.is_empty() {
            self.latest_label_timestamp = latest_label_timestamp;
            let mut s = self.get_new_string();
            write_timestamp_to_string(&mut s, latest_label_timestamp, self.utc_offset, self.print_seconds);
            self.label_strings.push_back(s);
        }

        let mut label_timestamp = latest_label_timestamp;
        while deque_index > 0 {
            deque_index -= 1;
            label_timestamp -= labels_on_multiples_of;
            let s = &mut self.label_strings[deque_index];
            write_timestamp_to_string(s, label_timestamp, self.utc_offset, self.print_seconds);
        }
    }

    fn ensure_has_labels(&mut self, oldest_label_timestamp: i64, latest_label_timestamp: i64) {
        let labels_on_multiples_of = self.labels_on_multiples_of as i64;

        let len_minus_one = self.label_strings.len() as i64 - 1;
        let mut current_oldest_label_timestamp = self.latest_label_timestamp - len_minus_one * labels_on_multiples_of;

        // Remove any label strings too old
        while current_oldest_label_timestamp < oldest_label_timestamp {
            let s = self.label_strings.pop_front();
            self.string_recycle_bin = self.string_recycle_bin.take().or(s);
            current_oldest_label_timestamp += labels_on_multiples_of;
        }

        // Remove any label strings too new
        while self.latest_label_timestamp > latest_label_timestamp {
            let s = self.label_strings.pop_back();
            self.string_recycle_bin = self.string_recycle_bin.take().or(s);
            current_oldest_label_timestamp -= labels_on_multiples_of;
        }

        // Add any missing old label strings
        while current_oldest_label_timestamp > oldest_label_timestamp {
            current_oldest_label_timestamp -= labels_on_multiples_of;
            let mut s = self.get_new_string();
            write_timestamp_to_string(&mut s, current_oldest_label_timestamp, self.utc_offset, self.print_seconds);
            self.label_strings.push_front(s);
        }

        // Add any missing new label strings
        while latest_label_timestamp > self.latest_label_timestamp {
            self.latest_label_timestamp += labels_on_multiples_of;
            let mut s = self.get_new_string();
            write_timestamp_to_string(&mut s, self.latest_label_timestamp, self.utc_offset, self.print_seconds);
            self.label_strings.push_back(s);
        }
    }

    fn render(&mut self, latest_timestamp: i64, unit_size_seconds: u32, area: Rect, buf: &mut Buffer) {
        self.recalculate_if_needed(unit_size_seconds, latest_timestamp);

        let unit_size_seconds = unit_size_seconds as i64;
        let labels_on_multiples_of = self.labels_on_multiples_of as i64;
        let markers_on_multiples_of = self.markers_on_multiples_of as i64;

        let mut content_i = buf.index_of(area.right() - 1, area.y);
        let ch = match latest_timestamp % markers_on_multiples_of {
            0 => CROSS_CHAR,
            _ => HORIZONTAL_CHAR,
        };
        buf.content[content_i].set_char(ch).set_style(AXIS_STYLE);

        let mut timestamp_i = (latest_timestamp - 1) / unit_size_seconds * unit_size_seconds;
        for _ in 0..(area.width - 11) {
            let ch = match timestamp_i % markers_on_multiples_of {
                0 => CROSS_CHAR,
                _ => HORIZONTAL_CHAR,
            };
            timestamp_i -= unit_size_seconds;

            content_i -= 1;
            buf.content[content_i].set_char(ch).set_style(AXIS_STYLE);
        }

        let oldest_char_timestamp = latest_timestamp - (area.width as i64 - 10) * unit_size_seconds;
        let oldest_label_timestamp = (oldest_char_timestamp + labels_on_multiples_of - 1) / labels_on_multiples_of * labels_on_multiples_of;
        let latest_label_timestamp = latest_timestamp / labels_on_multiples_of * labels_on_multiples_of;
        self.ensure_has_labels(oldest_label_timestamp, latest_label_timestamp);

        let latest_label_x = area.right() - 1 - (latest_timestamp - latest_label_timestamp) as u16;

        let y = area.y + 1;
        let mut label_x = latest_label_x;
        let label_spacing = (labels_on_multiples_of / unit_size_seconds) as u16;

        for label in self.label_strings.iter().rev() {
            buf.get_mut(label_x, y).set_char(VERTICAL_CHAR).set_style(AXIS_STYLE);
            let width = area.right().saturating_sub(label_x + 1) as usize;
            if width != 0 {
                buf.set_stringn(label_x + 1, y, label, width, LABEL_STYLE);
            }
            label_x -= label_spacing;
        }
    }
}

pub struct UsageGraph {
    controller: Rc<Controller>,
    background_task: JoinHandle<()>,
    vertical_axis: VerticalAxis,
    horizontal_aixs: HorizontalAxis,
    current_area: Rect,
}

impl Drop for UsageGraph {
    fn drop(&mut self) {
        self.background_task.abort();
    }
}

impl UsageGraph {
    pub fn new(redraw_notify: Rc<Notify>) -> Self {
        let controller = Rc::new(Controller::new(redraw_notify));

        let controller_weak = Rc::downgrade(&controller);
        let background_task = tokio::task::spawn_local(async move {
            background_ticker(controller_weak).await;
        });

        Self {
            controller,
            background_task,
            vertical_axis: VerticalAxis::new(),
            horizontal_aixs: HorizontalAxis::new(),
            current_area: Rect::default(),
        }
    }

    pub fn record_usage(&mut self, timestamp: i64, bytes_sent: u64, bytes_received: u64) {
        let mut inner = self.controller.inner.borrow_mut();
        inner.history.record_usage(timestamp, bytes_sent, bytes_received);
        self.controller.redraw_notify.notify_one();
    }
}

async fn background_ticker(controller: Weak<Controller>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        let rc = match controller.upgrade() {
            Some(rc) => rc,
            None => return,
        };

        let timestamp_now = time::OffsetDateTime::now_utc().unix_timestamp();
        let mut inner = rc.inner.borrow_mut();
        inner.history.record_usage(timestamp_now, 0, 0);
        rc.redraw_notify.notify_one();
    }
}

#[inline]
fn render_graph_plot(history: &VecDeque<UsageMeasure>, record_count: usize, max: u64, area: Rect, buf: &mut Buffer) {
    if max == 0 {
        return;
    }

    let mut x = area.right() - 1;
    for measure in history.iter().rev().take(record_count) {
        let (send_height, receive_height) = match (measure.sent, measure.received) {
            (0, 0) => (0, 0),
            (s, 0) => ((s * 2 * area.height as u64 / max).max(1), 0),
            (0, r) => (0, (r * 2 * area.height as u64 / max).max(1)),
            (bytes_sent, bytes_received) => {
                let desired_height = ((bytes_sent + bytes_received) * 2 * area.height as u64 / max).max(1);
                let mut send_height = (bytes_sent * area.height as u64 / max).max(1) * 2;
                let mut receive_height = desired_height.saturating_sub(send_height);
                if receive_height == 0 {
                    send_height -= 2;
                    receive_height = 1;
                }

                (send_height, receive_height)
            }
        };

        let mut y = area.bottom() - 1;

        let mut remaining_send_height = send_height;
        loop {
            let (ch, height_minus) = match remaining_send_height {
                0 => break,
                1 => (HALF_BLOCK_CHAR, 1),
                _ => (BLOCK_CHAR, 2),
            };
            remaining_send_height -= height_minus;

            buf.get_mut(x, y).set_char(ch).set_fg(SEND_COLOR);
            y -= 1;
        }

        let mut remaining_receive_height = receive_height;
        loop {
            let (ch, height_minus) = match remaining_receive_height {
                0 => break,
                1 => (HALF_BLOCK_CHAR, 1),
                _ => (BLOCK_CHAR, 2),
            };
            remaining_receive_height -= height_minus;

            buf.get_mut(x, y).set_char(ch).set_fg(RECEIVE_COLOR);
            y -= 1;
        }

        x -= 1;
    }
}

impl UIElement for UsageGraph {
    fn resize(&mut self, area: Rect) {
        let (_previous_width, previous_height) = (self.current_area.width, self.current_area.height);
        self.current_area = area;

        if previous_height != area.height {
            self.vertical_axis.resize(area.height - 1);
        }
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        if area.width < VERTICAL_LABELS_AREA_WIDTH + 2 || area.height < 3 {
            return;
        }

        let plot_area = Rect::new(
            area.x + VERTICAL_LABELS_AREA_WIDTH + 1,
            area.y,
            area.width - (VERTICAL_LABELS_AREA_WIDTH + 1),
            area.height - 2,
        );

        let mut inner_guard = self.controller.inner.borrow_mut();
        let inner = inner_guard.deref_mut();

        let history = inner.history.get_usage_by_unit(1);
        let record_count = history.len().min(plot_area.width as usize);
        let max = history.iter().rev().take(record_count).map(|m| m.sum()).max().unwrap_or(0);
        let max = pretty_round_max_bytes_value(max);

        let buf = frame.buffer_mut();
        render_graph_plot(history, record_count, max, plot_area, buf);

        let vertical_axis_area = Rect::new(area.x, area.y, VERTICAL_LABELS_AREA_WIDTH + 1, area.height - 1);
        self.vertical_axis.render(max, vertical_axis_area, buf);

        let horizontal_axis_area = Rect::new(area.x, area.bottom() - 2, area.width, 2);
        self.horizontal_aixs
            .render(inner.history.get_latest_timestamp(), 1, horizontal_axis_area, buf);
    }

    fn handle_event(&mut self, _event: &event::Event, _is_focused: bool) -> HandleEventStatus {
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        false
    }

    fn focus_lost(&mut self) {}
}
