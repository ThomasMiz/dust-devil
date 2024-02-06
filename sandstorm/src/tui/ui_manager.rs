use std::{
    future,
    rc::{Rc, Weak},
};

use crossterm::event::{self, KeyCode, KeyEventKind, MouseEventKind};
use dust_devil_core::{
    logging::{self, EventData},
    sandstorm::Metrics,
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
    menu_bar::MenuBar,
    popups::confirm_close_popup::ConfirmClosePopup,
    ui_element::{HandleEventStatus, PassFocusDirection, UIElement},
};

pub struct UIManager<W: AsyncWrite + Unpin + 'static> {
    redraw_notify: Rc<Notify>,
    current_area: Rect,
    shutdown_notify: Rc<Notify>,
    buffer_size_watch: broadcast::Sender<u32>,
    metrics_watch: watch::Sender<Metrics>,
    menu_bar: MenuBar<W>,
    bottom_area: BottomArea,
    focused_element: FocusedElement,
    popup_receiver: mpsc::UnboundedReceiver<Popup>,
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

enum FocusedElement {
    None,
    MenuBar,
    BottomArea,
    Popup(Vec<Popup>),
}

async fn receive_popup_close_index(focused_element: &mut FocusedElement) -> usize {
    if let FocusedElement::Popup(popups) = focused_element {
        let (_, recv_index) = recv_many_with_index(popups, |popup| &mut popup.close_receiver).await;
        recv_index
    } else {
        future::pending().await
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIManager<W> {
    pub fn new(
        manager: Weak<MutexedSandstormRequestManager<W>>,
        metrics: Metrics,
        redraw_notify: Rc<Notify>,
        shutdown_notify: Rc<Notify>,
    ) -> Self {
        let (buffer_size_watch, _) = broadcast::channel(1);
        let (metrics_watch, _metrics_watch_receiver) = watch::channel(metrics);
        let (popup_sender, popup_receiver) = mpsc::unbounded_channel();

        let menu_bar = MenuBar::new(
            Weak::clone(&manager),
            Rc::clone(&redraw_notify),
            buffer_size_watch.clone(),
            popup_sender,
        );
        let bottom_area = BottomArea::new(Rc::clone(&redraw_notify));

        Self {
            redraw_notify,
            current_area: Rect::default(),
            shutdown_notify,
            buffer_size_watch,
            metrics_watch,
            menu_bar,
            bottom_area,
            focused_element: FocusedElement::None,
            popup_receiver,
        }
    }

    fn push_popup(&mut self, mut popup: Popup) {
        if let FocusedElement::Popup(popups) = &mut self.focused_element {
            if let Some(last) = popups.last_mut() {
                last.element.focus_lost();
            }
            popup.element.receive_focus((0, 0));
            popups.push(popup);
        } else {
            match self.focused_element {
                FocusedElement::BottomArea => self.bottom_area.focus_lost(),
                FocusedElement::MenuBar => self.menu_bar.focus_lost(),
                _ => {}
            };

            let mut popups = Vec::with_capacity(4);
            popup.element.receive_focus((0, 0));
            popups.push(popup);
            self.focused_element = FocusedElement::Popup(popups);
        }

        self.redraw_notify.notify_one();
    }

    pub async fn background_process(&mut self) {
        select! {
            received = self.popup_receiver.recv() => {
                if let Some(p) = received {
                    self.push_popup(p);
                }
            }
            popup_index = receive_popup_close_index(&mut self.focused_element) => {
                let popups = match &mut self.focused_element {
                    FocusedElement::Popup(p) => p,
                    _ => unreachable!(),
                };

                let is_last = popup_index + 1 == popups.len();
                popups.remove(popup_index);
                if let Some(last) = popups.last_mut() {
                    if is_last {
                        last.element.receive_focus((0, 0));
                    }
                } else {
                    self.focused_element = FocusedElement::None;
                }

                self.redraw_notify.notify_one();
            }
        }
    }

    pub fn handle_terminal_event(&mut self, event: &event::Event) {
        if self.handle_termina_event_internal(event) {
            return;
        }

        // If the event was not handled and Esc was pressed, exit the TUI.
        if let event::Event::Key(key_event) = event {
            if key_event.code == KeyCode::Esc && key_event.kind != KeyEventKind::Release {
                self.push_popup(ConfirmClosePopup::new(self.redraw_notify.clone(), self.shutdown_notify.clone()).into());
            }
        }
    }

    fn handle_termina_event_internal(&mut self, event: &event::Event) -> bool {
        match &mut self.focused_element {
            FocusedElement::MenuBar => {
                let status = self.menu_bar.handle_event(event, true);

                match status {
                    HandleEventStatus::Handled => true,
                    HandleEventStatus::Unhandled => self.bottom_area.handle_event(event, false) == HandleEventStatus::Handled,
                    HandleEventStatus::PassFocus(focus_position, direction) => {
                        match direction {
                            PassFocusDirection::Forward | PassFocusDirection::Down => {
                                if self.bottom_area.receive_focus(focus_position) {
                                    self.menu_bar.focus_lost();
                                    self.focused_element = FocusedElement::BottomArea;
                                }
                            }
                            PassFocusDirection::Left | PassFocusDirection::Right | PassFocusDirection::Up => {}
                            PassFocusDirection::Away => {
                                self.menu_bar.focus_lost();
                                self.focused_element = FocusedElement::None;
                            }
                        }

                        true
                    }
                }
            }
            FocusedElement::BottomArea => {
                let status = self.bottom_area.handle_event(event, true);

                match status {
                    HandleEventStatus::Handled => true,
                    HandleEventStatus::Unhandled => self.menu_bar.handle_event(event, false) == HandleEventStatus::Handled,
                    HandleEventStatus::PassFocus(focus_position, direction) => {
                        match direction {
                            PassFocusDirection::Forward | PassFocusDirection::Up => {
                                if self.menu_bar.receive_focus(focus_position) {
                                    self.bottom_area.focus_lost();
                                    self.focused_element = FocusedElement::MenuBar
                                }
                            }
                            PassFocusDirection::Left | PassFocusDirection::Right | PassFocusDirection::Down => {}
                            PassFocusDirection::Away => {
                                self.bottom_area.focus_lost();
                                self.focused_element = FocusedElement::None;
                            }
                        }
                        true
                    }
                }
            }
            FocusedElement::Popup(popups) => {
                let mut is_focused = true;
                for popup in popups.iter_mut().rev() {
                    if popup.element.handle_event(event, is_focused) != HandleEventStatus::Unhandled {
                        break;
                    }
                    is_focused = false;
                }

                true
            }
            FocusedElement::None => match event {
                event::Event::Key(key_event) => {
                    let menubar_receives_focus = match key_event.code {
                        KeyCode::Left => Some((true, (self.current_area.width, 0))),
                        KeyCode::Right | KeyCode::Tab => Some((true, (0, 0))),
                        KeyCode::Up => Some((false, (0, self.current_area.height))),
                        KeyCode::Down => Some((true, (0, 0))),
                        _ => None,
                    };

                    match menubar_receives_focus {
                        Some((true, focus_position)) => {
                            if self.menu_bar.receive_focus(focus_position) {
                                self.focused_element = FocusedElement::MenuBar;
                                true
                            } else if self.bottom_area.receive_focus(focus_position) {
                                self.focused_element = FocusedElement::BottomArea;
                                true
                            } else {
                                false
                            }
                        }
                        Some((false, focus_position)) => {
                            if self.bottom_area.receive_focus(focus_position) {
                                self.focused_element = FocusedElement::BottomArea;
                                true
                            } else if self.menu_bar.receive_focus(focus_position) {
                                self.focused_element = FocusedElement::MenuBar;
                                true
                            } else {
                                false
                            }
                        }
                        None => {
                            self.menu_bar.handle_event(event, false) == HandleEventStatus::Handled
                                || self.bottom_area.handle_event(event, false) == HandleEventStatus::Handled
                        }
                    }
                }
                event::Event::Mouse(mouse_event) => {
                    if (mouse_event.kind == MouseEventKind::ScrollUp || mouse_event.kind == MouseEventKind::ScrollDown)
                        && self.bottom_area.receive_focus((mouse_event.column, mouse_event.row))
                    {
                        self.focused_element = FocusedElement::BottomArea;
                        self.bottom_area.handle_event(event, true);
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
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

        self.bottom_area.new_stream_event(event);
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        self.current_area = frame.size();

        let mut menu_area = frame.size();
        menu_area.height = menu_area.height.min(2);
        self.menu_bar.render(menu_area, frame.buffer_mut());

        let mut bottom_area = frame.size();
        bottom_area.y += 2;
        bottom_area.height = bottom_area.height.saturating_sub(2);

        self.bottom_area.render(bottom_area, frame.buffer_mut());

        if let FocusedElement::Popup(popups) = &mut self.focused_element {
            for popup in popups {
                popup.element.render(frame.size(), frame.buffer_mut());
            }
        }
    }
}
