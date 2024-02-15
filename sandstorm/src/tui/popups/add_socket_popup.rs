use std::{
    cell::RefCell,
    net::{AddrParseError, SocketAddr},
    rc::{Rc, Weak},
    str::FromStr,
    time::Duration,
};

use crossterm::event;
use ratatui::{
    layout::Rect,
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
        elements::{
            centered_text::CenteredText,
            dual_buttons::DualButtonsHandler,
            padded::Padded,
            text_entry::{CursorPosition, TextEntry, TextEntryController, TextEntryHandler},
            OnEnterResult,
        },
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

const SOCKS5_TITLE: &str = "─Add Socks5 Socket";
const SANDSTORM_TITLE: &str = "─Add Sandstorm Socket";
const OFFER_MESSAGE: &str = "Enter the address of the new socket (e.g. 127.0.0.1:1234 or [::1]:1234)";

const OPENING_MESSAGE: &str = "Opening socket...";

const BACKGROUND_COLOR: Color = Color::Green;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightGreen;
const TEXT_COLOR: Color = Color::Black;

const SERVER_ADD_ERROR_MESSAGE: &str = "The server encountered an error while opening the socket:";
const POPUP_WIDTH: u16 = 46;
const BIG_ERROR_POPUP_WIDTH: u16 = 48;
const ERROR_POPUP_WIDTH: u16 = 40;

struct ControllerInner {
    base: YesNoControllerInner,
    current_task: Option<JoinHandle<()>>,
    is_beeping_red: bool,
    is_doing_request: bool,
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
    socket_type: SocketPopupType,
    sockets_watch: broadcast::Sender<(SocketAddr, bool)>,
    popup_sender: mpsc::UnboundedSender<Popup>,
}

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        socket_type: SocketPopupType,
        sockets_watch: broadcast::Sender<(SocketAddr, bool)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (base, close_receiver) = YesNoControllerInner::new(redraw_notify, true);

        let inner = ControllerInner {
            base,
            current_task: None,
            is_beeping_red: false,
            is_doing_request: false,
        };

        let value = Self {
            inner: RefCell::new(inner),
            manager,
            socket_type,
            sockets_watch,
            popup_sender,
        };

        (value, close_receiver)
    }

    fn text_entry_beep_red(self: &Rc<Self>, text_controller: &Rc<TextEntryController>) {
        const BEEP_DELAY_MILLIS: u64 = 200;

        let mut inner = self.inner.borrow_mut();
        inner.is_beeping_red = true;
        inner.base.base.redraw_notify();
        drop(inner);

        let original_idle_bg = text_controller.get_idle_style().bg;
        let original_typing_bg = text_controller.get_typing_style().bg;

        let self_rc = Rc::clone(self);
        let text_rc = Rc::clone(text_controller);
        let handle = tokio::task::spawn_local(async move {
            for _ in 0..2 {
                text_rc.modify_idle_style(|style| style.bg = Some(Color::Red));
                text_rc.modify_typing_style(|style| style.bg = Some(Color::Red));
                tokio::time::sleep(Duration::from_millis(BEEP_DELAY_MILLIS)).await;
                text_rc.modify_idle_style(|style| style.bg = Some(BACKGROUND_COLOR));
                text_rc.modify_typing_style(|style| style.bg = Some(BACKGROUND_COLOR));
                tokio::time::sleep(Duration::from_millis(BEEP_DELAY_MILLIS)).await;
            }

            text_rc.modify_idle_style(|style| style.bg = original_idle_bg);
            text_rc.modify_typing_style(|style| style.bg = original_typing_bg);

            self_rc.inner.borrow_mut().is_beeping_red = false;
        });

        self.inner.borrow_mut().current_task = Some(handle);
    }

    fn perform_request(self: &Rc<Self>, socket_address: SocketAddr) {
        let mut inner = self.inner.borrow_mut();
        inner.is_doing_request = true;
        inner.base.set_showing_buttons(false);
        inner.base.base.set_closable(false);
        drop(inner);

        let self_weak = Rc::downgrade(self);
        let handle = tokio::task::spawn_local(async move {
            add_socket_task(self_weak, socket_address).await;
        });

        self.inner.borrow_mut().current_task = Some(handle);
    }

    fn on_yes_selected(self: &Rc<Self>, text_controller: &Rc<TextEntryController>) -> bool {
        let inner = self.inner.borrow();
        if inner.is_beeping_red || inner.is_doing_request {
            return false;
        }
        drop(inner);

        let parse_result: Result<SocketAddr, AddrParseError> = text_controller.with_text(|text| text.parse());
        match parse_result {
            Ok(socket_address) => self.perform_request(socket_address),
            Err(_) => self.text_entry_beep_red(text_controller),
        }

        parse_result.is_ok()
    }
}

async fn add_socket_task<W: AsyncWrite + Unpin + 'static>(controller: Weak<Controller<W>>, socket_address: SocketAddr) {
    let (manager_rc, socket_type) = match controller.upgrade().map(|rc| (rc.manager.upgrade(), rc)) {
        Some((Some(manager_rc), rc)) => (manager_rc, rc.socket_type),
        _ => return,
    };

    let result = socket_type.add_socket(manager_rc, socket_address).await;

    let rc = match controller.upgrade() {
        Some(rc) => rc,
        None => return,
    };

    let popup = match result {
        Ok(Ok(())) => {
            let _ = rc.sockets_watch.send((socket_address, true));
            rc.close_popup();
            return;
        }
        Ok(Err(error)) => MessagePopup::error_message(
            Rc::clone(&rc.inner.borrow().base.base.redraw_notify),
            ERROR_POPUP_TITLE.into(),
            SERVER_ADD_ERROR_MESSAGE.into(),
            BIG_ERROR_POPUP_WIDTH,
            Padded::new(Padding::new(0, 0, 0, 1), CenteredText::new(error.to_string().into(), Style::new())),
        )
        .into(),
        Err(_) => MessagePopup::empty_error_message(
            Rc::clone(&rc.inner.borrow().base.base.redraw_notify),
            ERROR_POPUP_TITLE.into(),
            REQUEST_SEND_ERROR_MESSAGE.into(),
            ERROR_POPUP_WIDTH,
        )
        .into(),
    };

    let _ = rc.popup_sender.send(popup);
    let mut inner = rc.inner.borrow_mut();
    inner.is_doing_request = false;
    inner.base.base.set_closable(true);
    inner.base.set_showing_buttons(true);
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

pub struct AddSocketPopup<W: AsyncWrite + Unpin + 'static> {
    base: YesNoPopup<Controller<W>, Padded<TextEntry<TextHandler<W>>>, ButtonHandler<W>>,
}

struct TextHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> TextEntryHandler for TextHandler<W> {
    fn on_enter(&mut self, controller: &Rc<TextEntryController>) -> OnEnterResult {
        match self.controller.on_yes_selected(controller) {
            true => OnEnterResult::PassFocusAway,
            false => OnEnterResult::Handled,
        }
    }

    fn on_char(&mut self, _controller: &Rc<TextEntryController>, c: char, _cursor: &CursorPosition) -> bool {
        c.is_ascii_hexdigit() || ['.', ':', '%', '[', ']'].contains(&c)
    }

    fn on_text_changed(&mut self, controller: &Rc<TextEntryController>) -> bool {
        let background_color = controller.with_text(|text| match <SocketAddr as FromStr>::from_str(text) {
            Ok(_) => Color::Green,
            Err(_) => Color::Red,
        });

        controller.modify_typing_style(|style| *style = style.bg(background_color));
        true
    }
}

struct ButtonHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
    text_controller: Rc<TextEntryController>,
}

impl<W: AsyncWrite + Unpin + 'static> DualButtonsHandler for ButtonHandler<W> {
    fn on_left(&mut self) {
        self.controller.on_yes_selected(&self.text_controller);
    }

    fn on_right(&mut self) {
        self.controller.close_popup();
    }
}

impl<W: AsyncWrite + Unpin + 'static> AddSocketPopup<W> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        socket_type: SocketPopupType,
        sockets_watch: broadcast::Sender<(SocketAddr, bool)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = Controller::new(Rc::clone(&redraw_notify), manager, socket_type, sockets_watch, popup_sender);
        let controller = Rc::new(controller);

        let text_style = Style::new().fg(TEXT_COLOR);
        let selected_text_style = text_style.bg(SELECTED_BACKGROUND_COLOR);

        let text_entry = TextEntry::new(
            Rc::clone(&redraw_notify),
            String::new(),
            text_style,
            selected_text_style,
            45,
            TextHandler {
                controller: Rc::clone(&controller),
            },
        );

        let button_handler = ButtonHandler {
            controller: Rc::clone(&controller),
            text_controller: text_entry.controller(),
        };

        let content = Padded::new(Padding::horizontal(1), text_entry);

        let title = match socket_type {
            SocketPopupType::Socks5 => SOCKS5_TITLE,
            SocketPopupType::Sandstorm => SANDSTORM_TITLE,
        };

        let base = YesNoPopup::new(
            redraw_notify,
            title.into(),
            OFFER_MESSAGE.into(),
            text_style,
            1,
            YES_TITLE.into(),
            CANCEL_TITLE.into(),
            text_style,
            selected_text_style,
            text_style,
            selected_text_style,
            OPENING_MESSAGE.into(),
            text_style,
            TEXT_COLOR,
            BACKGROUND_COLOR,
            SizeConstraint::new().max(POPUP_WIDTH, u16::MAX),
            controller,
            content,
            button_handler,
        );

        let value = Self { base };
        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for AddSocketPopup<W> {
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
