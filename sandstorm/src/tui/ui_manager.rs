use std::rc::Rc;

use crossterm::event::{self, KeyCode};
use dust_devil_core::{
    logging::{self, EventData},
    sandstorm::Metrics,
};
use ratatui::Frame;
use tokio::{
    io::AsyncWrite,
    sync::{broadcast, oneshot, watch, Notify},
};

use crate::sandstorm::MutexedSandstormRequestManager;

use super::{
    log_block::LogBlock,
    menu_bar::MenuBar,
    ui_element::{HandleEventStatus, UIElement},
};

pub struct UIManager<W: AsyncWrite + Unpin + 'static> {
    _manager: Rc<MutexedSandstormRequestManager<W>>,
    shutdown_sender: Option<oneshot::Sender<()>>,
    buffer_size_watch: broadcast::Sender<u32>,
    metrics_watch: watch::Sender<Metrics>,
    menu_bar: MenuBar,
    log_block: LogBlock,
}

impl<W: AsyncWrite + Unpin + 'static> UIManager<W> {
    pub fn new(
        manager: Rc<MutexedSandstormRequestManager<W>>,
        metrics: Metrics,
        redraw_notify: Rc<Notify>,
        shutdown_sender: oneshot::Sender<()>,
    ) -> Self {
        let (buffer_size_watch, _) = broadcast::channel(1);
        let (metrics_watch, _metrics_watch_receiver) = watch::channel(metrics);

        let menu_bar = MenuBar::new(manager.clone(), redraw_notify.clone(), buffer_size_watch.clone());
        let log_block = LogBlock::new(redraw_notify);

        Self {
            _manager: manager,
            shutdown_sender: Some(shutdown_sender),
            buffer_size_watch,
            metrics_watch,
            menu_bar,
            log_block,
        }
    }

    pub fn handle_terminal_event(&mut self, event: &event::Event) {
        // TODO: Implement focus passing
        if self.log_block.handle_event(event, true) != HandleEventStatus::Unhandled {
            return;
        }

        if let event::Event::Key(key_event) = event {
            if key_event.code == KeyCode::Esc {
                if let Some(shutdown_sender) = self.shutdown_sender.take() {
                    let _ = shutdown_sender.send(());
                }
            }
        }
    }

    pub fn handle_stream_event(&mut self, event: logging::Event) {
        match event.data {
            EventData::NewClientConnectionAccepted(_, _) => {
                self.metrics_watch.send_modify(|metrics| {
                    metrics.current_client_connections += 1;
                    metrics.historic_client_connections += 1;
                });
            }
            EventData::ClientConnectionFinished(_, _, _, _) => {
                self.metrics_watch.send_modify(|metrics| {
                    metrics.current_client_connections -= 1;
                });
            }
            EventData::ClientBytesSent(_, count) => {
                self.metrics_watch.send_modify(|metrics| {
                    metrics.client_bytes_sent += count;
                });
            }
            EventData::ClientBytesReceived(_, count) => {
                self.metrics_watch.send_modify(|metrics| {
                    metrics.client_bytes_received += count;
                });
            }
            EventData::NewSandstormConnectionAccepted(_, _) => {
                self.metrics_watch.send_modify(|metrics| {
                    metrics.current_sandstorm_connections += 1;
                    metrics.historic_sandstorm_connections += 1;
                });
            }
            EventData::SandstormConnectionFinished(_, _) => {
                self.metrics_watch.send_modify(|metrics| {
                    metrics.current_sandstorm_connections -= 1;
                });
            }
            EventData::BufferSizeChangedByManager(_, buffer_size) => {
                let _ = self.buffer_size_watch.send(buffer_size);
            }
            _ => {}
        }

        self.log_block.new_stream_event(event);
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let mut menu_area = frame.size();
        menu_area.height = menu_area.height.min(2);
        self.menu_bar.render(menu_area, frame.buffer_mut());

        let mut bottom_area = frame.size();
        bottom_area.y += 2;
        bottom_area.height = bottom_area.height.saturating_sub(2);

        self.log_block.render(bottom_area, frame.buffer_mut())

        /*frame.render_widget(
            Block::new()
                .border_type(ratatui::widgets::BorderType::Double)
                .borders(Borders::ALL)
                .style(Style::reset().red())
                .title("Pedro."),
            bottom_area,
        );*/
    }
}
