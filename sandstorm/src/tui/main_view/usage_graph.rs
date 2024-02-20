use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    rc::{Rc, Weak},
    time::Duration,
};

use crossterm::event;
use ratatui::{layout::Rect, style::Color, symbols::bar::HALF, Frame};
use time::OffsetDateTime;
use tokio::{sync::Notify, task::JoinHandle, time::MissedTickBehavior};

use crate::tui::ui_element::{HandleEventStatus, UIElement};

use super::usage_tracker::UsageTracker;

pub const MAX_HISTORY_SECONDS: usize = 60 * 60 * 24 * 1; // 1 day

const BLOCK_CHAR: char = '█';
const HALF_BLOCK_CHAR: char = '▄';

const VERTICAL_LABELS_MAX_LENGTH: u16 = 6;
const VERTICAL_CHAR: char = '│';
const HORIZONTAL_CHAR: char = '─';
const VERTICAL_CROSS_CHAR: char = '┼';
const SPACE_BETWEEN_HORIZONTAL_MARKERS: u16 = 6;
const SPACE_BETWEEN_VERTICAL_MARKERS: u16 = 2;

const SEND_COLOR: Color = Color::LightGreen;
const RECEIVE_COLOR: Color = Color::LightBlue;

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

pub struct UsageGraph {
    controller: Rc<Controller>,
    background_task: JoinHandle<()>,
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

impl UIElement for UsageGraph {
    fn resize(&mut self, area: Rect) {
        self.current_area = area;
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let mut inner_guard = self.controller.inner.borrow_mut();
        let inner = inner_guard.deref_mut();

        let history_vec = inner.history.get_usage_by_unit(3);
        let record_count = history_vec.len().min(area.width as usize);
        let max = history_vec.iter().rev().take(record_count).map(|m| m.sum()).max().unwrap_or(0);
        if max == 0 {
            return;
        }

        let buf = frame.buffer_mut();
        let mut x = area.right() - 1;
        for measure in history_vec.iter().rev().take(record_count) {
            let (send_height, receive_height) = match (measure.sent, measure.received) {
                (0, 0) => (0, 0),
                (s, 0) => ((s * 2 * area.height as u64 / max).max(1), 0),
                (0, r) => (0, (r * 2 * area.height as u64 / max).max(1)),
                (bytes_sent, bytes_received) => {
                    let desired_height = ((bytes_sent + bytes_received) * 2 * area.height as u64 / max).max(1);
                    let send_height = (bytes_sent * area.height as u64 / max).max(1) * 2;
                    let mut receive_height = desired_height.saturating_sub(send_height).max(1);

                    let overflow = (send_height + receive_height).saturating_sub(2 * area.height as u64);
                    receive_height -= overflow;

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

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        false
    }

    fn focus_lost(&mut self) {}
}
