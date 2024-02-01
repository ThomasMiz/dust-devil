use std::rc::Rc;

use crossterm::event::{self, KeyCode, KeyEventKind, MouseEventKind};
use dust_devil_core::{
    logging::{self, EventData},
    sandstorm::Metrics,
};
use ratatui::{layout::Rect, Frame};
use tokio::{
    io::AsyncWrite,
    sync::{broadcast, oneshot, watch, Notify},
};

use crate::sandstorm::MutexedSandstormRequestManager;

use super::{
    bottom_area::BottomArea,
    menu_bar::MenuBar,
    ui_element::{HandleEventStatus, PassFocusDirection, UIElement},
};

pub struct UIManager<W: AsyncWrite + Unpin + 'static> {
    _manager: Rc<MutexedSandstormRequestManager<W>>,
    shutdown_sender: Option<oneshot::Sender<()>>,
    buffer_size_watch: broadcast::Sender<u32>,
    metrics_watch: watch::Sender<Metrics>,
    menu_bar: MenuBar,
    bottom_area: BottomArea,
    current_area: Rect,
    focused_element: FocusedElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedElement {
    None,
    MenuBar,
    BottomArea,
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
        let bottom_area = BottomArea::new(redraw_notify);

        Self {
            _manager: manager,
            shutdown_sender: Some(shutdown_sender),
            buffer_size_watch,
            metrics_watch,
            menu_bar,
            bottom_area,
            current_area: Rect::default(),
            focused_element: FocusedElement::None,
        }
    }

    pub fn handle_terminal_event(&mut self, event: &event::Event) {
        if self.handle_termina_event_internal(event) {
            return;
        }

        // If the event was not handled and Esc was pressed, exit the TUI.
        if let event::Event::Key(key_event) = event {
            if key_event.code == KeyCode::Esc && key_event.kind != KeyEventKind::Release {
                if let Some(shutdown_sender) = self.shutdown_sender.take() {
                    let _ = shutdown_sender.send(());
                }
            }
        }
    }

    fn handle_termina_event_internal(&mut self, event: &event::Event) -> bool {
        match self.focused_element {
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

        self.bottom_area.render(bottom_area, frame.buffer_mut())

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
