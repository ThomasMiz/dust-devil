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
use time::OffsetDateTime;
use tokio::{sync::Notify, task::JoinHandle, time::MissedTickBehavior};

use crate::tui::{
    pretty_print::PrettyByteDisplayer,
    ui_element::{HandleEventStatus, UIElement},
};

use super::usage_tracker::{UsageMeasure, UsageTracker};

pub const MAX_HISTORY_SECONDS: usize = 60 * 60 * 24 * 1; // 1 day

const BLOCK_CHAR: char = '█';
const HALF_BLOCK_CHAR: char = '▄';

const MARKER_ZERO: &str = "0B/s";

const VERTICAL_LABELS_AREA_WIDTH: u16 = 9;
const VERTICAL_CHAR: char = '│';
const HORIZONTAL_CHAR: char = '─';
const CROSS_CHAR: char = '┼';
const SPACE_BETWEEN_HORIZONTAL_MARKERS: u16 = 6;

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

pub struct UsageGraph {
    controller: Rc<Controller>,
    background_task: JoinHandle<()>,
    vertical_axis: VerticalAxis,
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

        let history = inner.history.get_usage_by_unit(3);
        let record_count = history.len().min(plot_area.width as usize);
        let max = history.iter().rev().take(record_count).map(|m| m.sum()).max().unwrap_or(0);
        let max = pretty_round_max_bytes_value(max);

        let buf = frame.buffer_mut();
        render_graph_plot(history, record_count, max, plot_area, buf);

        let vertical_axis_area = Rect::new(area.x, area.y, VERTICAL_LABELS_AREA_WIDTH + 1, area.height - 1);
        self.vertical_axis.render(max, vertical_axis_area, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        false
    }

    fn focus_lost(&mut self) {}
}
