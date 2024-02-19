use std::{
    cell::RefCell,
    net::SocketAddr,
    rc::{Rc, Weak},
};

use crossterm::event;
use dust_devil_core::sandstorm::RemoveSocketResponse;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::Padding,
    Frame,
};
use tokio::{
    io::AsyncWrite,
    sync::{broadcast, mpsc, oneshot, Notify},
    task::JoinHandle,
};

use crate::{
    sandstorm::MutexedSandstormRequestManager,
    tui::{
        elements::{dual_buttons::DualButtonsHandler, padded::Padded, text::Text},
        ui_element::{HandleEventStatus, UIElement},
        ui_manager::Popup,
    },
};

use super::{
    message_popup::{MessagePopup, ERROR_POPUP_TITLE, REQUEST_SEND_ERROR_MESSAGE},
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
    sockets_popup::SocketPopupType,
    yes_no_popup::{YesNoControllerInner, YesNoPopup, YesNoPopupController},
    CANCEL_TITLE, YES_TITLE,
};

const SOCKS5_TITLE: &str = "─Close Socks5 Socket";
const SANDSTORM_TITLE: &str = "─Close Sandstorm Socket";
const SOCKS5_PROMPT: &str =
    "Are you sure you want to close this socket? The server will no longer listen for incoming socks5 connections at:";
const SANDSTORM_PROMPT: &str =
    "Are you sure you want to close this socket? The server will no longer listen for incoming Sandstorm connections at:";
const POPUP_WIDTH: u16 = 46;

const CLOSING_MESSAGE: &str = "Closing socket...";

const BACKGROUND_COLOR: Color = Color::Green;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightGreen;
const TEXT_COLOR: Color = Color::Black;

const SERVER_REMOVE_ERROR_MESSAGE: &str = "The server could not find the socket you requested to remove.";
const ERROR_POPUP_WIDTH: u16 = 40;

struct ControllerInner {
    base: YesNoControllerInner,
    current_task: Option<JoinHandle<()>>,
}

impl Drop for ControllerInner {
    fn drop(&mut self) {
        if let Some(handle) = self.current_task.take() {
            handle.abort();
        }
    }
}

struct Controller<W: AsyncWrite + Unpin + 'static> {
    inner: RefCell<ControllerInner>,
    manager: Weak<MutexedSandstormRequestManager<W>>,
    socket_address: SocketAddr,
    socket_type: SocketPopupType,
    sockets_watch: broadcast::Sender<(SocketAddr, bool)>,
    popup_sender: mpsc::UnboundedSender<Popup>,
}

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        socket_address: SocketAddr,
        socket_type: SocketPopupType,
        sockets_watch: broadcast::Sender<(SocketAddr, bool)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (base, close_receiver) = YesNoControllerInner::new(redraw_notify, true);

        let inner = ControllerInner { base, current_task: None };

        let value = Self {
            inner: RefCell::new(inner),
            manager,
            socket_address,
            socket_type,
            sockets_watch,
            popup_sender,
        };

        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> PopupBaseController for Controller<W> {
    fn redraw_notify(&self) {
        self.inner.borrow_mut().base.base.redraw_notify();
    }

    fn request_resize(&self) {
        self.inner.borrow_mut().base.base.request_resize();
    }

    fn get_resize_requested(&self) -> bool {
        self.inner.borrow_mut().base.base.get_resize_requested()
    }

    fn close_popup(&self) {
        self.inner.borrow_mut().base.base.close_popup();
    }

    fn set_closable(&self, closable: bool) {
        self.inner.borrow_mut().base.base.set_closable(closable);
    }

    fn get_closable(&self) -> bool {
        self.inner.borrow_mut().base.base.get_closable()
    }
}

impl<W: AsyncWrite + Unpin + 'static> YesNoPopupController for Controller<W> {
    fn set_showing_buttons(&self, showing: bool) {
        self.inner.borrow_mut().base.set_showing_buttons(showing);
    }

    fn get_showing_buttons(&self) -> bool {
        self.inner.borrow_mut().base.get_showing_buttons()
    }
}

async fn close_socket_task<W: AsyncWrite + Unpin + 'static>(controller_weak: Weak<Controller<W>>) {
    let (manager_rc, socket_type, socket_address) = match controller_weak.upgrade().map(|rc| (rc.manager.upgrade(), rc)) {
        Some((Some(manager_rc), rc)) => (manager_rc, rc.socket_type, rc.socket_address),
        _ => return,
    };

    let result = socket_type.remove_socket(manager_rc, socket_address).await;

    let rc = match controller_weak.upgrade() {
        Some(rc) => rc,
        None => return,
    };

    match result {
        Ok(RemoveSocketResponse::Ok) => {
            let _ = rc.sockets_watch.send((socket_address, false));
            rc.close_popup();
        }
        Ok(RemoveSocketResponse::SocketNotFound) | Err(_) => {
            let prompt_str = match result.is_err() {
                true => REQUEST_SEND_ERROR_MESSAGE,
                false => SERVER_REMOVE_ERROR_MESSAGE,
            };

            let popup = MessagePopup::empty_error_message(
                Rc::clone(&rc.inner.borrow().base.base.redraw_notify),
                ERROR_POPUP_TITLE.into(),
                prompt_str.into(),
                ERROR_POPUP_WIDTH,
            );

            let _ = rc.popup_sender.send(popup.into());

            let mut inner = rc.inner.borrow_mut();
            inner.base.base.set_closable(true);
            inner.base.set_showing_buttons(true);
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn on_yes_selected(self: &Rc<Self>) {
        let mut inner = self.inner.borrow_mut();
        inner.base.base.set_closable(false);
        inner.base.set_showing_buttons(false);
        drop(inner);

        let controller_weak = Rc::downgrade(self);
        let handle = tokio::task::spawn_local(async move {
            close_socket_task(controller_weak).await;
        });

        self.inner.borrow_mut().current_task = Some(handle);
    }
}

pub struct CloseSocketPopup<W: AsyncWrite + Unpin + 'static> {
    base: YesNoPopup<Controller<W>, Padded<Text>, ButtonHandler<W>>,
}

struct ButtonHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> DualButtonsHandler for ButtonHandler<W> {
    fn on_left(&mut self) {
        self.controller.on_yes_selected();
    }

    fn on_right(&mut self) {
        self.controller.close_popup();
    }
}

impl<W: AsyncWrite + Unpin + 'static> CloseSocketPopup<W> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        socket_address: SocketAddr,
        socket_type: SocketPopupType,
        sockets_watch: broadcast::Sender<(SocketAddr, bool)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = Controller::new(
            Rc::clone(&redraw_notify),
            manager,
            socket_address,
            socket_type,
            sockets_watch,
            popup_sender,
        );
        let controller = Rc::new(controller);

        let (title, prompt_str) = match socket_type {
            SocketPopupType::Socks5 => (SOCKS5_TITLE, SOCKS5_PROMPT),
            SocketPopupType::Sandstorm => (SANDSTORM_TITLE, SANDSTORM_PROMPT),
        };

        let text_style = Style::new().fg(TEXT_COLOR);
        let selected_text_style = text_style.bg(SELECTED_BACKGROUND_COLOR);

        let button_handler = ButtonHandler {
            controller: Rc::clone(&controller),
        };

        let content = Text::new(socket_address.to_string().into(), text_style, Alignment::Center);

        let base = YesNoPopup::new(
            redraw_notify,
            title.into(),
            prompt_str.into(),
            text_style,
            1,
            YES_TITLE.into(),
            CANCEL_TITLE.into(),
            text_style,
            selected_text_style,
            text_style,
            selected_text_style,
            CLOSING_MESSAGE.into(),
            text_style,
            TEXT_COLOR,
            BACKGROUND_COLOR,
            SizeConstraint::new(POPUP_WIDTH, u16::MAX),
            controller,
            Padded::new(Padding::horizontal(1), content),
            button_handler,
        );

        let value = Self { base };
        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for CloseSocketPopup<W> {
    fn resize(&mut self, area: Rect) {
        self.base.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.base.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.base.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.base.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.base.focus_lost();
    }
}
