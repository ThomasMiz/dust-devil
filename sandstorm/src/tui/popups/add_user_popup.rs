use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    rc::{Rc, Weak},
    time::Duration,
};

use crossterm::event;
use dust_devil_core::{sandstorm::AddUserResponse, users::UserRole};
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

const TITLE: &str = "─Add User";
const PROMPT_MESSAGE: &str = "Enter the information of the user to add:";

const ADDING_MESSAGE: &str = "Adding user...";

const BACKGROUND_COLOR: Color = Color::Green;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightGreen;
const TEXT_COLOR: Color = Color::Black;

const SERVER_ADD_ERROR_MESSAGE: &str = "The user was not added because the server rejected the operation:";
const SERVER_INVALID_VALUES_MESSAGE: &str = "The provided values are invalid";
const SERVER_ALREADY_EXISTS_MESSAGE: &str = "A user with said username already exists";
const POPUP_WIDTH: u16 = 46;
const BIG_ERROR_POPUP_WIDTH: u16 = 48;
const ERROR_POPUP_WIDTH: u16 = 40;

const ROLE_SELECTOR_LABEL: &str = "Role:";
const ROLE_REGULAR_STR: &str = "[REGULAR]";
const ROLE_REGULAR_SHORTCUT: Option<char> = Some('1');
const ROLE_ADMIN_STR: &str = "[ADMIN]";
const ROLE_ADMIN_SHORTCUT: Option<char> = Some('2');
const USERNAME_ENTRY_LABEL: &str = "Username:";
const USERNAME_ENTRY_MAX_LENGTH: usize = 255;
const PASSWORD_ENTRY_LABEL: &str = "Password:";
const PASSWORD_ENTRY_MAX_LENGTH: usize = 255;

struct ControllerInner {
    base: YesNoControllerInner,
    current_task: Option<JoinHandle<()>>,
    username_entry_controller: Weak<TextEntryController>,
    password_entry_controller: Weak<TextEntryController>,
    selected_role: UserRole,
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
    users_watch: broadcast::Sender<(UserNotificationType, String, UserRole)>,
    popup_sender: mpsc::UnboundedSender<Popup>,
}

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        users_watch: broadcast::Sender<(UserNotificationType, String, UserRole)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (base, close_receiver) = YesNoControllerInner::new(redraw_notify, true);

        let inner = ControllerInner {
            base,
            current_task: None,
            username_entry_controller: Weak::new(),
            password_entry_controller: Weak::new(),
            selected_role: UserRole::Regular,
            is_beeping_red: false,
            is_doing_request: false,
        };

        let value = Self {
            inner: RefCell::new(inner),
            manager,
            users_watch,
            popup_sender,
        };

        (value, close_receiver)
    }

    fn set_text_entry_controllers(
        &self,
        username_entry_controller: Weak<TextEntryController>,
        password_entry_controller: Weak<TextEntryController>,
    ) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        inner.username_entry_controller = username_entry_controller;
        inner.password_entry_controller = password_entry_controller;
    }

    fn text_entry_beep_red(self: &Rc<Self>, text_controller: Rc<TextEntryController>) {
        const BEEP_DELAY_MILLIS: u64 = 200;

        let mut inner = self.inner.borrow_mut();
        inner.is_beeping_red = true;
        inner.base.base.redraw_notify();
        drop(inner);

        let original_idle_bg = text_controller.get_idle_style().bg;
        let original_typing_bg = text_controller.get_typing_style().bg;

        let self_rc = Rc::clone(self);
        let handle = tokio::task::spawn_local(async move {
            for _ in 0..2 {
                text_controller.modify_idle_style(|style| style.bg = Some(Color::Red));
                text_controller.modify_typing_style(|style| style.bg = Some(Color::Red));
                tokio::time::sleep(Duration::from_millis(BEEP_DELAY_MILLIS)).await;
                text_controller.modify_idle_style(|style| style.bg = Some(BACKGROUND_COLOR));
                text_controller.modify_typing_style(|style| style.bg = Some(BACKGROUND_COLOR));
                tokio::time::sleep(Duration::from_millis(BEEP_DELAY_MILLIS)).await;
            }

            text_controller.modify_idle_style(|style| style.bg = original_idle_bg);
            text_controller.modify_typing_style(|style| style.bg = original_typing_bg);

            self_rc.inner.borrow_mut().is_beeping_red = false;
        });

        self.inner.borrow_mut().current_task = Some(handle);
    }

    fn perform_request(self: &Rc<Self>, username: String, password: String, role: UserRole) {
        let mut inner = self.inner.borrow_mut();
        inner.is_doing_request = true;
        inner.base.set_showing_buttons(false);
        inner.base.base.set_closable(false);
        drop(inner);

        let self_weak = Rc::downgrade(self);
        let handle = tokio::task::spawn_local(async move {
            add_user_task(self_weak, username, password, role).await;
        });

        self.inner.borrow_mut().current_task = Some(handle);
    }

    fn on_yes_selected(self: &Rc<Self>) -> bool {
        let inner = self.inner.borrow();
        if inner.is_beeping_red || inner.is_doing_request {
            return false;
        }

        let username_controller = match inner.username_entry_controller.upgrade() {
            Some(rc) => rc,
            None => return false,
        };
        let password_controller = match inner.password_entry_controller.upgrade() {
            Some(rc) => rc,
            None => return false,
        };

        let selected_role = inner.selected_role;
        drop(inner);

        let username = username_controller.with_text(|text| String::from(text));
        if username.is_empty() {
            self.text_entry_beep_red(username_controller);
            return false;
        }

        let password = password_controller.with_text(|text| String::from(text));
        if password.is_empty() {
            self.text_entry_beep_red(password_controller);
            return false;
        }

        self.perform_request(username, password, selected_role);
        true
    }

    fn selected_role_changed(&self, selected_role: UserRole) {
        let mut inner = self.inner.borrow_mut();
        inner.selected_role = selected_role;
    }
}

async fn add_user_task<W: AsyncWrite + Unpin + 'static>(
    controller: Weak<Controller<W>>,
    username: String,
    password: String,
    role: UserRole,
) {
    let manager_rc = match controller.upgrade().map(|rc| rc.manager.upgrade()) {
        Some(Some(manager_rc)) => manager_rc,
        _ => return,
    };

    let (response_sender, response_receiver) = oneshot::channel();
    let send_status = manager_rc
        .add_user_fn(&username, &password, role, |result| {
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
        Some(AddUserResponse::Ok) => {
            let _ = rc.users_watch.send((UserNotificationType::Registered, username, role));
            rc.close_popup();
            return;
        }
        Some(error_response) => {
            let error_message = match error_response {
                AddUserResponse::AlreadyExists => SERVER_ALREADY_EXISTS_MESSAGE,
                AddUserResponse::InvalidValues => SERVER_INVALID_VALUES_MESSAGE,
                AddUserResponse::Ok => unreachable!(),
            };

            let text = Text::new(error_message.into(), Style::new(), Alignment::Center);
            MessagePopup::error_message(
                Rc::clone(&rc.inner.borrow().base.base.redraw_notify),
                ERROR_POPUP_TITLE.into(),
                SERVER_ADD_ERROR_MESSAGE.into(),
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

pub struct AddUserPopup<W: AsyncWrite + Unpin + 'static> {
    base: YesNoPopup<Controller<W>, Padded<Content<W>>, ButtonHandler<W>>,
}

type RoleSelectorLine<W> = HorizontalSplit<TextLine, ArrowSelector<RoleSelectorHandler<W>>>;
type UsernameEntryLine<W> = HorizontalSplit<TextLine, TextEntry<TextHandler<W>>>;
type PasswordEntryLine<W> = HorizontalSplit<TextLine, TextEntry<TextHandler<W>>>;

struct Content<W: AsyncWrite + Unpin + 'static> {
    inner: VerticalSplit<RoleSelectorLine<W>, VerticalSplit<UsernameEntryLine<W>, PasswordEntryLine<W>>>,
}

impl<W: AsyncWrite + Unpin + 'static> Content<W> {
    fn new(redraw_notify: Rc<Notify>, controller: Rc<Controller<W>>) -> Self {
        let text_style = Style::new().fg(TEXT_COLOR);
        let selected_text_style = text_style.bg(SELECTED_BACKGROUND_COLOR);

        let role_selector = ArrowSelector::new(
            Rc::clone(&redraw_notify),
            vec![
                (ROLE_REGULAR_STR.into(), ROLE_REGULAR_SHORTCUT),
                (ROLE_ADMIN_STR.into(), ROLE_ADMIN_SHORTCUT),
            ],
            0,
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

        let username_entry = TextEntry::new(
            Rc::clone(&redraw_notify),
            String::new(),
            text_style,
            selected_text_style,
            USERNAME_ENTRY_MAX_LENGTH,
            TextHandler {
                controller: Rc::clone(&controller),
            },
        );
        let username_entry_controller = Rc::downgrade(username_entry.deref());
        let username_label = TextLine::new(USERNAME_ENTRY_LABEL.into(), text_style, Alignment::Left);
        let username_line = HorizontalSplit::new(username_label, username_entry, 0, 1);

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

        controller.set_text_entry_controllers(username_entry_controller, password_entry_controller);

        let bottom_split = VerticalSplit::new(username_line, password_line, 1, 0);
        let inner = VerticalSplit::new(role_selector_line, bottom_split, 1, 0);
        Self { inner }
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for Content<W> {
    fn resize(&mut self, area: Rect) {
        self.inner.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.inner.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        let controller = &self.inner.upper.right.handler.controller;
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

impl<W: AsyncWrite + Unpin + 'static> AddUserPopup<W> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        users_watch: broadcast::Sender<(UserNotificationType, String, UserRole)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = Controller::new(Rc::clone(&redraw_notify), manager, users_watch, popup_sender);
        let controller = Rc::new(controller);

        let button_handler = ButtonHandler {
            controller: Rc::clone(&controller),
        };

        let text_style = Style::new().fg(TEXT_COLOR);
        let selected_text_style = text_style.bg(SELECTED_BACKGROUND_COLOR);

        let content_inner = Content::new(Rc::clone(&redraw_notify), Rc::clone(&controller));
        let content = Padded::new(Padding::horizontal(1), content_inner);

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
            ADDING_MESSAGE.into(),
            text_style,
            TEXT_COLOR,
            BACKGROUND_COLOR,
            SizeConstraint::new(POPUP_WIDTH, u16::MAX),
            controller,
            content,
            button_handler,
        );

        let value = Self { base };
        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for AddUserPopup<W> {
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
