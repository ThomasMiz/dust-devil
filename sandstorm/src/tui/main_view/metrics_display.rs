use std::{fmt::Write, rc::Rc};

use crossterm::event;
use dust_devil_core::sandstorm::Metrics;
use ratatui::{layout::Rect, style::Style, Frame};
use tokio::sync::Notify;

use crate::tui::{
    pretty_print::PrettyByteDisplayer,
    ui_element::{HandleEventStatus, UIElement},
};

pub const HEIGHT: u16 = 6;
pub const MIN_WIDTH: u16 = 25;

const CURRENT_CLIENTS_LABEL: &str = "Current clients:";
const HISTORIC_CLIENTS_LABEL: &str = "Historic clients:";
const BYTES_SENT_LABEL: &str = "Bytes sent:";
const BYTES_RECEIVED_LABEL: &str = "Bytes received:";
const CURRENT_MANAGERS_LABEL: &str = "Current managers:";
const HISTORIC_MANAGERS_LABEL: &str = "Historic managers:";

pub struct MetricsDisplay {
    redraw_notify: Rc<Notify>,
    metrics: Metrics,
    current_client_connections_str: String,
    historic_client_connections_str: String,
    client_bytes_sent_str: String,
    client_bytes_received_str: String,
    current_sandstorm_connections_str: String,
    historic_sandstorm_connections_str: String,
}

impl MetricsDisplay {
    pub fn new(redraw_notify: Rc<Notify>, metrics: Metrics) -> Self {
        Self {
            redraw_notify,
            metrics,
            current_client_connections_str: String::with_capacity(20),
            historic_client_connections_str: String::with_capacity(20),
            client_bytes_sent_str: String::with_capacity(20),
            client_bytes_received_str: String::with_capacity(20),
            current_sandstorm_connections_str: String::with_capacity(20),
            historic_sandstorm_connections_str: String::with_capacity(20),
        }
    }

    pub fn on_new_client_connection_accepted(&mut self) {
        self.metrics.current_client_connections += 1;
        self.metrics.historic_client_connections += 1;
        self.current_client_connections_str.clear();
        self.historic_client_connections_str.clear();
        self.redraw_notify.notify_one();
    }

    pub fn on_client_connection_finished(&mut self) {
        self.metrics.current_client_connections -= 1;
        self.current_client_connections_str.clear();
        self.redraw_notify.notify_one();
    }

    pub fn on_client_bytes_sent(&mut self, count: u64) {
        self.metrics.client_bytes_sent += count;
        self.client_bytes_sent_str.clear();
        self.redraw_notify.notify_one();
    }

    pub fn on_client_bytes_received(&mut self, count: u64) {
        self.metrics.client_bytes_received += count;
        self.client_bytes_received_str.clear();
        self.redraw_notify.notify_one();
    }

    pub fn on_new_sandstorm_collection_accepted(&mut self) {
        self.metrics.current_sandstorm_connections += 1;
        self.metrics.historic_sandstorm_connections += 1;
        self.current_sandstorm_connections_str.clear();
        self.historic_sandstorm_connections_str.clear();
        self.redraw_notify.notify_one();
    }

    pub fn on_sandstorm_collection_finished(&mut self) {
        self.metrics.current_sandstorm_connections -= 1;
        self.current_sandstorm_connections_str.clear();
        self.redraw_notify.notify_one();
    }

    fn update_labels_where_needed(&mut self) {
        if self.current_client_connections_str.is_empty() {
            let _ = write!(self.current_client_connections_str, "{}", self.metrics.current_client_connections);
        }

        if self.historic_client_connections_str.is_empty() {
            let _ = write!(self.historic_client_connections_str, "{}", self.metrics.historic_client_connections);
        }

        if self.client_bytes_sent_str.is_empty() {
            let b = self.metrics.client_bytes_sent;
            let _ = write!(self.client_bytes_sent_str, "{} ({})", PrettyByteDisplayer(b as usize), b);
        }

        if self.client_bytes_received_str.is_empty() {
            let b = self.metrics.client_bytes_received;
            let _ = write!(self.client_bytes_received_str, "{} ({})", PrettyByteDisplayer(b as usize), b);
        }

        if self.current_sandstorm_connections_str.is_empty() {
            let c = self.metrics.current_sandstorm_connections;
            let _ = write!(self.current_sandstorm_connections_str, "{}", c);
        }

        if self.historic_sandstorm_connections_str.is_empty() {
            let c = self.metrics.historic_sandstorm_connections;
            let _ = write!(self.historic_sandstorm_connections_str, "{}", c);
        }
    }
}

impl UIElement for MetricsDisplay {
    fn resize(&mut self, _area: Rect) {}

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.update_labels_where_needed();

        let labels = [
            (CURRENT_CLIENTS_LABEL, &self.current_client_connections_str),
            (HISTORIC_CLIENTS_LABEL, &self.historic_client_connections_str),
            (BYTES_SENT_LABEL, &self.client_bytes_sent_str),
            (BYTES_RECEIVED_LABEL, &self.client_bytes_received_str),
            (CURRENT_MANAGERS_LABEL, &self.current_sandstorm_connections_str),
            (HISTORIC_MANAGERS_LABEL, &self.historic_sandstorm_connections_str),
        ];

        let style = Style::new();
        let buf = frame.buffer_mut();
        let mut y = area.y;
        for (label, value) in labels.into_iter().take(area.height as usize) {
            let (mut x, _) = buf.set_stringn(area.x, y, label, area.width as usize, style);
            x += 1;
            if x < area.right() {
                buf.set_stringn(x, y, value, (area.right() - x) as usize, style);
            }
            y += 1;
        }
    }

    fn handle_event(&mut self, _event: &event::Event, _is_focused: bool) -> HandleEventStatus {
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        false
    }

    fn focus_lost(&mut self) {}
}
