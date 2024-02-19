use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    rc::{Rc, Weak},
};

use crossterm::event;
use dust_devil_core::{sandstorm::UpdateUserResponse, users::UserRole};
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
        elements::{
            arrow_selector::{ArrowSelector, ArrowSelectorHandler},
            dual_buttons::DualButtonsHandler,
            horizontal_split::HorizontalSplit,
            padded::Padded,
            text::{Text, TextLine},
            text_entry::{CursorPosition, TextEntry, TextEntryController, TextEntryHandler},
            vertical_split::VerticalSplit,
            OnEnterResult,
        },
        ui_element::{AutosizeUIElement, HandleEventStatus, PassFocusDirection, UIElement},
        ui_manager::{Popup, UserNotificationType},
    },
};

use super::{
    message_popup::{MessagePopup, ERROR_POPUP_TITLE, REQUEST_SEND_ERROR_MESSAGE},
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
    yes_no_popup::{YesNoControllerInner, YesNoPopup, YesNoPopupController},
    CANCEL_TITLE, CONFIRM_TITLE,
};

const TITLE: &str = "─Update User";
const PROMPT_MESSAGE: &str = "Modify the information you want to update:";

const UPDATING_MESSAGE: &str = "Updating user...";

const BACKGROUND_COLOR: Color = Color::Green;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightGreen;
const TEXT_COLOR: Color = Color::Black;

const SERVER_UPDATE_ERROR_MESSAGE: &str = "The user was not updated because the server rejected the operation:";
const USER_NOT_FOUND_MESSAGE: &str = "No user was found with such username. Was it just deleted?";
const CANNOT_DELETE_ONLY_ADMIN_MESSAGE: &str = "Cannot delete the only admin, that would leave the server inaccessible!";
const NOTHING_WAS_REQUESTED_MESSAGE: &str = "No changes were requested";
const POPUP_WIDTH: u16 = 46;
const BIG_ERROR_POPUP_WIDTH: u16 = 48;
const ERROR_POPUP_WIDTH: u16 = 40;

const ROLE_SELECTOR_LABEL: &str = "Role:";
const ROLE_REGULAR_STR: &str = "[REGULAR]";
const ROLE_REGULAR_SHORTCUT: Option<char> = Some('1');
const ROLE_ADMIN_STR: &str = "[ADMIN]";
const ROLE_ADMIN_SHORTCUT: Option<char> = Some('2');
const USERNAME_LABEL: &str = "Username:";
const PASSWORD_ENTRY_LABEL: &str = "Password:";
const PASSWORD_ENTRY_MAX_LENGTH: usize = 255;

struct ControllerInner {
    base: YesNoControllerInner,
    user_role: UserRole,
    did_user_role_change: bool,
    current_task: Option<JoinHandle<()>>,
    password_entry_controller: Weak<TextEntryController>,
    selected_role: Option<UserRole>,
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
    username: String,
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

        let inner = ControllerInner {
            base,
            user_role: user.1,
            did_user_role_change: false,
            current_task: None,
            password_entry_controller: Weak::new(),
            selected_role: None,
            is_beeping_red: false,
            is_doing_request: false,
        };

        let value = Self {
            inner: RefCell::new(inner),
            manager,
            username: user.0,
            users_watch,
            popup_sender,
        };

        (value, close_receiver)
    }

    fn set_text_entry_controller(&self, password_entry_controller: Weak<TextEntryController>) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        inner.password_entry_controller = password_entry_controller;
    }

    fn perform_request(self: &Rc<Self>, password: Option<String>, role: Option<UserRole>) {
        let mut inner = self.inner.borrow_mut();
        inner.is_doing_request = true;
        inner.base.set_showing_buttons(false);
        inner.base.base.set_closable(false);
        drop(inner);

        let self_weak = Rc::downgrade(self);
        let handle = tokio::task::spawn_local(async move {
            update_user_task(self_weak, password, role).await;
        });

        self.inner.borrow_mut().current_task = Some(handle);
    }

    fn on_yes_selected(self: &Rc<Self>) -> bool {
        let inner = self.inner.borrow();
        if inner.is_beeping_red || inner.is_doing_request {
            return false;
        }

        let password_controller = match inner.password_entry_controller.upgrade() {
            Some(rc) => rc,
            None => return false,
        };

        let password = password_controller.with_text(|text| match text.is_empty() {
            true => None,
            false => Some(String::from(text)),
        });

        let selected_role = inner.selected_role;
        drop(inner);

        self.perform_request(password, selected_role);
        true
    }

    fn user_role_changed(&self, user_role: UserRole) {
        let mut inner = self.inner.borrow_mut();
        inner.user_role = user_role;
        inner.did_user_role_change = inner.selected_role.is_none();
        inner.base.base.redraw_notify();
    }

    fn did_user_role_change(&self) -> Option<UserRole> {
        let mut inner = self.inner.borrow_mut();
        if inner.did_user_role_change {
            inner.did_user_role_change = false;
            Some(inner.user_role)
        } else {
            None
        }
    }

    fn selected_role_changed(&self, selected_role: UserRole) {
        let mut inner = self.inner.borrow_mut();
        inner.did_user_role_change = false;
        inner.selected_role = Some(selected_role);
    }
}

async fn update_user_task<W: AsyncWrite + Unpin + 'static>(
    controller: Weak<Controller<W>>,
    password: Option<String>,
    role: Option<UserRole>,
) {
    let controller_rc = match controller.upgrade() {
        Some(rc) => rc,
        None => return,
    };

    let manager_rc = match controller_rc.manager.upgrade() {
        Some(rc) => rc,
        _ => return,
    };

    let username = controller_rc.username.clone();
    drop(controller_rc);

    let (response_sender, response_receiver) = oneshot::channel();
    let send_status = manager_rc
        .update_user_fn(&username, password.as_deref(), role, |result| {
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
        Some(UpdateUserResponse::Ok) => {
            let current_role = role.unwrap_or_else(|| rc.inner.borrow().user_role);
            let _ = rc.users_watch.send((UserNotificationType::Updated, username, current_role));
            rc.close_popup();
            return;
        }
        Some(error_response) => {
            let error_message = match error_response {
                UpdateUserResponse::UserNotFound => USER_NOT_FOUND_MESSAGE,
                UpdateUserResponse::CannotDeleteOnlyAdmin => CANNOT_DELETE_ONLY_ADMIN_MESSAGE,
                UpdateUserResponse::NothingWasRequested => NOTHING_WAS_REQUESTED_MESSAGE,
                UpdateUserResponse::Ok => unreachable!(),
            };

            let text = Text::new(error_message.into(), Style::new(), Alignment::Center);
            MessagePopup::error_message(
                Rc::clone(&rc.inner.borrow().base.base.redraw_notify),
                ERROR_POPUP_TITLE.into(),
                SERVER_UPDATE_ERROR_MESSAGE.into(),
                BIG_ERROR_POPUP_WIDTH,
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

pub struct UpdateUserPopup<W: AsyncWrite + Unpin + 'static> {
    base: YesNoPopup<Controller<W>, Padded<Content<W>>, ButtonHandler<W>>,
    background_task: JoinHandle<()>,
}

impl<W: AsyncWrite + Unpin + 'static> Drop for UpdateUserPopup<W> {
    fn drop(&mut self) {
        self.background_task.abort();
    }
}

async fn background_update_user<W: AsyncWrite + Unpin + 'static>(
    controller_weak: Weak<Controller<W>>,
    mut users_watch: broadcast::Receiver<(UserNotificationType, String, UserRole)>,
) {
    loop {
        let (notification_type, username, role) = match users_watch.recv().await {
            Ok(t) => t,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return,
        };

        let controller_rc = match controller_weak.upgrade() {
            Some(rc) => rc,
            None => return,
        };

        if notification_type != UserNotificationType::Deleted && controller_rc.username.eq(&username) {
            controller_rc.user_role_changed(role);
        }
    }
}

type UsernameTextLine = HorizontalSplit<TextLine, Text>;
type RoleSelectorLine<W> = HorizontalSplit<TextLine, ArrowSelector<RoleSelectorHandler<W>>>;
type PasswordEntryLine<W> = HorizontalSplit<TextLine, TextEntry<TextHandler<W>>>;

struct Content<W: AsyncWrite + Unpin + 'static> {
    inner: VerticalSplit<UsernameTextLine, VerticalSplit<RoleSelectorLine<W>, PasswordEntryLine<W>>>,
}

impl<W: AsyncWrite + Unpin + 'static> Content<W> {
    fn new(redraw_notify: Rc<Notify>, controller: Rc<Controller<W>>) -> Self {
        let text_style = Style::new().fg(TEXT_COLOR);
        let selected_text_style = text_style.bg(SELECTED_BACKGROUND_COLOR);

        let username_text = Text::new(controller.username.clone().into(), text_style, Alignment::Left);
        let username_label = TextLine::new(USERNAME_LABEL.into(), text_style, Alignment::Left);
        let username_line = HorizontalSplit::new(username_label, username_text, 0, 1);

        let role_selected_index = match controller.inner.borrow().user_role {
            UserRole::Regular => 0,
            UserRole::Admin => 1,
        };

        let role_selector = ArrowSelector::new(
            Rc::clone(&redraw_notify),
            vec![
                (ROLE_REGULAR_STR.into(), ROLE_REGULAR_SHORTCUT),
                (ROLE_ADMIN_STR.into(), ROLE_ADMIN_SHORTCUT),
            ],
            role_selected_index,
            text_style,
            selected_text_style,
            selected_text_style,
            selected_text_style,
            false,
            RoleSelectorHandler {
                controller: Rc::clone(&controller),
            },
        );
        let role_selector_label = TextLine::new(ROLE_SELECTOR_LABEL.into(), text_style, Alignment::Left);
        let role_selector_line = HorizontalSplit::new(role_selector_label, role_selector, 0, 1);

        let password_entry = TextEntry::new(
            Rc::clone(&redraw_notify),
            String::new(),
            text_style,
            selected_text_style,
            PASSWORD_ENTRY_MAX_LENGTH,
            TextHandler {
                controller: Rc::clone(&controller),
            },
        );
        let password_entry_controller = Rc::downgrade(password_entry.deref());
        let password_label = TextLine::new(PASSWORD_ENTRY_LABEL.into(), text_style, Alignment::Left);
        let password_line = HorizontalSplit::new(password_label, password_entry, 0, 1);

        controller.set_text_entry_controller(password_entry_controller);

        let bottom_split = VerticalSplit::new(role_selector_line, password_line, 1, 0);
        let inner = VerticalSplit::new(username_line, bottom_split, 0, 0);
        Self { inner }
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for Content<W> {
    fn resize(&mut self, area: Rect) {
        self.inner.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let controller = &self.inner.lower.upper.right.handler.controller;
        if let Some(new_user_role) = controller.did_user_role_change() {
            let role_selector = &mut self.inner.lower.upper.right;
            role_selector.set_selected_index_no_redraw(match new_user_role {
                UserRole::Regular => 0,
                UserRole::Admin => 1,
            });
        }

        self.inner.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        let controller = &self.inner.lower.upper.right.handler.controller;
        if controller.inner.borrow().is_doing_request {
            return HandleEventStatus::Handled;
        }

        self.inner.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.inner.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.inner.focus_lost();
    }
}

impl<W: AsyncWrite + Unpin + 'static> AutosizeUIElement for Content<W> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        self.inner.begin_resize(width, height)
    }
}

struct RoleSelectorHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> ArrowSelectorHandler for RoleSelectorHandler<W> {
    fn selection_changed(&mut self, selected_index: usize) {
        let selected_role = match selected_index {
            1 => UserRole::Admin,
            _ => UserRole::Regular,
        };

        self.controller.selected_role_changed(selected_role);
    }
}

struct TextHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> TextEntryHandler for TextHandler<W> {
    fn on_enter(&mut self, _controller: &Rc<TextEntryController>) -> OnEnterResult {
        match self.controller.on_yes_selected() {
            true => OnEnterResult::PassFocus(PassFocusDirection::Away),
            false => OnEnterResult::Handled,
        }
    }

    fn on_char(&mut self, _controller: &Rc<TextEntryController>, _c: char, _cursor: &CursorPosition) -> bool {
        true
    }

    fn on_text_changed(&mut self, _controller: &Rc<TextEntryController>) -> bool {
        true
    }
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

impl<W: AsyncWrite + Unpin + 'static> UpdateUserPopup<W> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        user: (String, UserRole),
        users_watch: broadcast::Sender<(UserNotificationType, String, UserRole)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = Controller::new(Rc::clone(&redraw_notify), manager, user, users_watch, popup_sender);
        let controller = Rc::new(controller);

        let button_handler = ButtonHandler {
            controller: Rc::clone(&controller),
        };

        let text_style = Style::new().fg(TEXT_COLOR);
        let selected_text_style = text_style.bg(SELECTED_BACKGROUND_COLOR);

        let content_inner = Content::new(Rc::clone(&redraw_notify), Rc::clone(&controller));
        let content = Padded::new(Padding::horizontal(1), content_inner);

        let controller_weak = Rc::downgrade(&controller);
        let users_watch_receiver = controller.users_watch.subscribe();

        let base = YesNoPopup::new(
            redraw_notify,
            TITLE.into(),
            PROMPT_MESSAGE.into(),
            text_style,
            1,
            CONFIRM_TITLE.into(),
            CANCEL_TITLE.into(),
            text_style,
            selected_text_style,
            text_style,
            selected_text_style,
            UPDATING_MESSAGE.into(),
            text_style,
            TEXT_COLOR,
            BACKGROUND_COLOR,
            SizeConstraint::new(POPUP_WIDTH, u16::MAX),
            controller,
            content,
            button_handler,
        );

        let background_task = tokio::task::spawn_local(async move {
            background_update_user(controller_weak, users_watch_receiver).await;
        });

        let value = Self { base, background_task };
        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for UpdateUserPopup<W> {
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
