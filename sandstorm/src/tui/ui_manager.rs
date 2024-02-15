use std::{
    net::SocketAddr,
    rc::{Rc, Weak},
};

use crossterm::event::{self, KeyCode, KeyEventKind};
use dust_devil_core::{
    logging::{self, EventData},
    sandstorm::Metrics,
    socks5::AuthMethod,
};
use ratatui::{layout::Rect, Frame};
use tokio::{
    io::AsyncWrite,
    select,
    sync::{broadcast, mpsc, oneshot, watch, Notify},
};

use crate::{sandstorm::MutexedSandstormRequestManager, utils::futures::recv_many_with_index};

use super::{
    bottom_area::BottomArea,
    elements::{focus_cell::FocusCell, vertical_split::VerticalSplit},
    menu_bar::MenuBar,
    popups::confirm_close_popup::ConfirmClosePopup,
    ui_element::{HandleEventStatus, UIElement},
};

pub struct UIManager<W: AsyncWrite + Unpin + 'static> {
    redraw_notify: Rc<Notify>,
    current_area: Rect,
    shutdown_notify: Rc<Notify>,
    socks5_sockets_watch: broadcast::Sender<(SocketAddr, bool)>,
    sandstorm_sockets_watch: broadcast::Sender<(SocketAddr, bool)>,
    buffer_size_watch: broadcast::Sender<u32>,
    auth_methods_watch: broadcast::Sender<(AuthMethod, bool)>,
    metrics_watch: watch::Sender<Metrics>,
    root: FocusCell<VerticalSplit<MenuBar<W>, BottomArea>>,
    popup_receiver: mpsc::UnboundedReceiver<Popup>,
    popups: Vec<Popup>,
}

pub struct Popup {
    element: Box<dyn UIElement>,
    close_receiver: oneshot::Receiver<()>,
}

impl<T: UIElement + 'static> From<(T, oneshot::Receiver<()>)> for Popup {
    fn from(value: (T, oneshot::Receiver<()>)) -> Self {
        Popup {
            element: Box::new(value.0),
            close_receiver: value.1,
        }
    }
}

async fn receive_popup_close_index(popups: &mut [Popup]) -> usize {
    let (_, recv_index) = recv_many_with_index(popups, |popup| &mut popup.close_receiver).await;
    recv_index
}

impl<W: AsyncWrite + Unpin + 'static> UIManager<W> {
    pub fn new(
        manager: Weak<MutexedSandstormRequestManager<W>>,
        metrics: Metrics,
        redraw_notify: Rc<Notify>,
        shutdown_notify: Rc<Notify>,
    ) -> Self {
        let (buffer_size_watch, _) = broadcast::channel(1);
        let (auth_methods_watch, _) = broadcast::channel(32);
        let (socks5_sockets_watch, _) = broadcast::channel(32);
        let (sandstorm_sockets_watch, _) = broadcast::channel(32);
        let (metrics_watch, _metrics_watch_receiver) = watch::channel(metrics);
        let (popup_sender, popup_receiver) = mpsc::unbounded_channel();

        let menu_bar = MenuBar::new(
            Weak::clone(&manager),
            Rc::clone(&redraw_notify),
            socks5_sockets_watch.clone(),
            sandstorm_sockets_watch.clone(),
            buffer_size_watch.clone(),
            auth_methods_watch.clone(),
            popup_sender,
        );
        let bottom_area = BottomArea::new(Rc::clone(&redraw_notify));

        Self {
            redraw_notify,
            current_area: Rect::default(),
            shutdown_notify,
            buffer_size_watch,
            auth_methods_watch,
            socks5_sockets_watch,
            sandstorm_sockets_watch,
            metrics_watch,
            root: FocusCell::new(VerticalSplit::new(menu_bar, bottom_area, 2, 0)),
            popup_receiver,
            popups: Vec::new(),
        }
    }

    fn push_popup(&mut self, mut popup: Popup) {
        if let Some(last) = self.popups.last_mut() {
            last.element.focus_lost();
        } else {
            self.root.focus_lost();
        }

        popup.element.receive_focus((0, 0));
        popup.element.resize(self.current_area);
        self.popups.push(popup);

        self.redraw_notify.notify_one();
    }

    pub async fn background_process(&mut self) {
        select! {
            received = self.popup_receiver.recv() => {
                if let Some(p) = received {
                    self.push_popup(p);
                }
            }
            popup_index = receive_popup_close_index(&mut self.popups) => {
                let is_last = popup_index + 1 == self.popups.len();
                self.popups.remove(popup_index);
                if let Some(last) = self.popups.last_mut() {
                    if is_last {
                        last.element.receive_focus((0, 0));
                    }
                }

                self.redraw_notify.notify_one();
            }
        }
    }

    pub fn handle_terminal_event(&mut self, event: &event::Event) {
        if let Some(popup) = self.popups.last_mut() {
            popup.element.handle_event(event, true);
        } else if self.root.handle_event(event, true) == HandleEventStatus::Unhandled {
            if let event::Event::Key(key_event) = event {
                if key_event.code == KeyCode::Esc && key_event.kind != KeyEventKind::Release {
                    let redraw_notify = Rc::clone(&self.redraw_notify);
                    let shutdown_notify = Rc::clone(&self.shutdown_notify);
                    self.push_popup(ConfirmClosePopup::new(redraw_notify, shutdown_notify).into());
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
            EventData::NewSocks5Socket(socket_address) => {
                let _ = self.socks5_sockets_watch.send((socket_address, true));
            }
            EventData::RemovedSocks5Socket(socket_address) => {
                let _ = self.socks5_sockets_watch.send((socket_address, false));
            }
            EventData::NewSandstormSocket(socket_address) => {
                let _ = self.sandstorm_sockets_watch.send((socket_address, true));
            }
            EventData::RemovedSandstormSocket(socket_address) => {
                let _ = self.sandstorm_sockets_watch.send((socket_address, false));
            }
            EventData::BufferSizeChangedByManager(_, buffer_size) => {
                let _ = self.buffer_size_watch.send(buffer_size);
            }
            EventData::AuthMethodToggledByManager(_, auth_method, state) => {
                let _ = self.auth_methods_watch.send((auth_method, state));
            }
            _ => {}
        }

        self.root.lower.new_stream_event(event);
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let previous_area = self.current_area;
        self.current_area = frame.size();

        if self.current_area != previous_area {
            self.root.resize(self.current_area);

            for popup in self.popups.iter_mut() {
                popup.element.resize(self.current_area);
            }
        }

        self.root.render(self.current_area, frame);

        for popup in self.popups.iter_mut() {
            popup.element.render(self.current_area, frame);
        }
    }
}
