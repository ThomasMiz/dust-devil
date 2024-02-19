use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crossterm::event;
use dust_devil_core::{sandstorm::DeleteUserResponse, users::UserRole};
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
        ui_manager::{Popup, UserNotificationType},
    },
};

use super::{
    message_popup::{MessagePopup, ERROR_POPUP_TITLE, REQUEST_SEND_ERROR_MESSAGE},
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
    update_user_popup::{CANNOT_DELETE_ONLY_ADMIN_MESSAGE, USER_NOT_FOUND_MESSAGE},
    yes_no_popup::{YesNoControllerInner, YesNoPopup, YesNoPopupController},
    CANCEL_TITLE, YES_TITLE,
};

const TITLE: &str = "─Delete User";
const PROMPT: &str = "Are you sure you want to delete the following user? This action cannot be undone!";
const POPUP_WIDTH: u16 = 46;

const DELETING_MESSAGE: &str = "Deleting user...";

const BACKGROUND_COLOR: Color = Color::Blue;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightBlue;
const TEXT_COLOR: Color = Color::White;

const SERVER_DELETE_ERROR_MESSAGE: &str = "The user was not deleted because the server rejected the operation:";
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
    user: (String, UserRole),
    users_watch: broadcast::Sender<(UserNotificationType, String, UserRole)>,
    popup_sender: mpsc::UnboundedSender<Popup>,
}

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        user: (String, UserRole),
        users_watch: broadcast::Sender<(UserNotificationType, String, UserRole)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (base, close_receiver) = YesNoControllerInner::new(redraw_notify, true);

        let inner = ControllerInner { base, current_task: None };

        let value = Self {
            inner: RefCell::new(inner),
            manager,
            user,
            users_watch,
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

async fn delete_user_task<W: AsyncWrite + Unpin + 'static>(controller: Weak<Controller<W>>) {
    let controller_rc = match controller.upgrade() {
        Some(rc) => rc,
        None => return,
    };

    let manager_rc = match controller_rc.manager.upgrade() {
        Some(rc) => rc,
        _ => return,
    };

    let username = controller_rc.user.0.clone();
    drop(controller_rc);

    let (response_sender, response_receiver) = oneshot::channel();
    let send_status = manager_rc
        .delete_user_fn(&username, |result| {
            let _ = response_sender.send(result);
        })
        .await;
    drop(manager_rc);

    let maybe_result = if send_status.is_err() {
        None
    } else {
        match response_receiver.await {
            Ok(list) => Some(list),
            Err(_) => None,
        }
    };

    let rc = match controller.upgrade() {
        Some(rc) => rc,
        None => return,
    };

    let popup = match maybe_result {
        Some(DeleteUserResponse::Ok) => {
            let _ = rc.users_watch.send((UserNotificationType::Deleted, username, rc.user.1));
            rc.close_popup();
            return;
        }
        Some(delete_error) => {
            let error_str = match delete_error {
                DeleteUserResponse::UserNotFound => USER_NOT_FOUND_MESSAGE,
                DeleteUserResponse::CannotDeleteOnlyAdmin => CANNOT_DELETE_ONLY_ADMIN_MESSAGE,
                DeleteUserResponse::Ok => unreachable!(),
            };

            let text = Text::new(error_str.into(), Style::new(), Alignment::Center);
            MessagePopup::error_message(
                Rc::clone(&rc.inner.borrow().base.base.redraw_notify),
                ERROR_POPUP_TITLE.into(),
                SERVER_DELETE_ERROR_MESSAGE.into(),
                ERROR_POPUP_WIDTH,
                Padded::new(Padding::new(1, 1, 0, 1), text),
            )
            .into()
        }
        None => MessagePopup::empty_error_message(
            Rc::clone(&rc.inner.borrow().base.base.redraw_notify),
            ERROR_POPUP_TITLE.into(),
            REQUEST_SEND_ERROR_MESSAGE.into(),
            ERROR_POPUP_WIDTH,
        )
        .into(),
    };

    let _ = rc.popup_sender.send(popup);

    let mut inner = rc.inner.borrow_mut();
    inner.base.base.set_closable(true);
    inner.base.set_showing_buttons(true);
}

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn on_yes_selected(self: &Rc<Self>) {
        let mut inner = self.inner.borrow_mut();
        inner.base.base.set_closable(false);
        inner.base.set_showing_buttons(false);
        drop(inner);

        let controller_weak = Rc::downgrade(self);
        let handle = tokio::task::spawn_local(async move {
            delete_user_task(controller_weak).await;
        });

        self.inner.borrow_mut().current_task = Some(handle);
    }
}

pub struct DeleteUserPopup<W: AsyncWrite + Unpin + 'static> {
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

impl<W: AsyncWrite + Unpin + 'static> DeleteUserPopup<W> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        user: (String, UserRole),
        users_watch: broadcast::Sender<(UserNotificationType, String, UserRole)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = Controller::new(Rc::clone(&redraw_notify), manager, user, users_watch, popup_sender);
        let controller = Rc::new(controller);

        let text_style = Style::new().fg(TEXT_COLOR);
        let selected_text_style = text_style.bg(SELECTED_BACKGROUND_COLOR);

        let button_handler = ButtonHandler {
            controller: Rc::clone(&controller),
        };

        let content = Text::new(controller.user.0.clone().into(), text_style, Alignment::Center);

        let base = YesNoPopup::new(
            redraw_notify,
            TITLE.into(),
            PROMPT.into(),
            text_style,
            1,
            YES_TITLE.into(),
            CANCEL_TITLE.into(),
            text_style,
            selected_text_style,
            text_style,
            selected_text_style,
            DELETING_MESSAGE.into(),
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

impl<W: AsyncWrite + Unpin + 'static> UIElement for DeleteUserPopup<W> {
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
