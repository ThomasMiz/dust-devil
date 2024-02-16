use std::{
    cell::RefCell,
    cmp::Ordering,
    ops::{Deref, DerefMut},
    rc::{Rc, Weak},
};

use crossterm::event;
use dust_devil_core::users::UserRole;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
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
            centered_button::{ButtonHandler, CenteredButton},
            centered_text::{CenteredText, CenteredTextLine},
            horizontal_split::HorizontalSplit,
            long_list::{LongList, LongListHandler},
            padded::Padded,
            text_entry::{CursorPosition, TextEntry, TextEntryController, TextEntryHandler},
            OnEnterResult,
        },
        text_wrapper::{wrap_lines_by_chars, StaticString},
        ui_element::{AutosizeUIElement, HandleEventStatus, PassFocusDirection, UIElement},
        ui_manager::{Popup, UserNotificationType},
    },
};

use super::{
    loading_popup::{LoadingPopup, LoadingPopupController, LoadingPopupControllerInner},
    message_popup::REQUEST_SEND_ERROR_MESSAGE,
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
};

const BACKGROUND_COLOR: Color = Color::Cyan;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightCyan;
const TEXT_COLOR: Color = Color::Black;

const TITLE: &str = "─Users";
const LOADING_MESSAGE: &str = "Getting user list from the server...";
const LOADING_STYLE: Style = Style::new().fg(TEXT_COLOR);
const HELP_MESSAGE: &str = "Scroll the list with the arrow keys, press (ENTER) on a user to update or delete it.";
const HELP_MESSAGE_STYLE: Style = Style::new().fg(TEXT_COLOR);
const ADD_USER_BUTTON_TEXT: &str = "[add new user (a)]";
const ADD_USER_SHORTCUT_KEY: char = 'a';
const POPUP_WIDTH: u16 = 48;
const MAX_POPUP_HEIGHT: u16 = 24;

const TYPE_FILTER_LABEL: &str = "Type:";
const FILTER_ALL_STR: &str = "[ALL]";
const FILTER_ALL_SHORTCUT: Option<char> = Some('1');
const FILTER_REGULAR_STR: &str = "[REGULAR]";
const FILTER_REGULAR_SHORTCUT: Option<char> = Some('2');
const FILTER_ADMIN_STR: &str = "[ADMIN]";
const FILTER_ADMIN_SHORTCUT: Option<char> = Some('3');
const USERNAME_FILTER_LABEL: &str = "Username:";
const USERNAME_FILTER_MAX_LENGTH: usize = 256;

const MIN_USER_LIST_HEIGHT: usize = 8;

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum UserTypeFilter {
    All = 0,
    Regular = 1,
    Admin = 2,
}

struct ControllerInner {
    base: LoadingPopupControllerInner,
    users: Vec<(String, UserRole)>,
    did_list_change: bool,
    type_filter: UserTypeFilter,
    username_filter: String,
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
        let (base, close_receiver) = LoadingPopupControllerInner::new(redraw_notify, true, true);

        let inner = ControllerInner {
            base,
            users: Vec::new(),
            did_list_change: false,
            type_filter: UserTypeFilter::All,
            username_filter: String::new(),
        };

        let value = Self {
            inner: RefCell::new(inner),
            manager,
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

impl<W: AsyncWrite + Unpin + 'static> LoadingPopupController for Controller<W> {
    fn set_is_loading(&self, loading: bool) {
        self.inner.borrow_mut().base.set_is_loading(loading);
    }

    fn get_is_loading(&self) -> bool {
        self.inner.borrow().base.get_is_loading()
    }

    fn set_new_loading_text(&self, text: StaticString) {
        self.inner.borrow_mut().base.set_new_loading_text(text);
    }

    fn get_new_loading_text(&self) -> Option<StaticString> {
        self.inner.borrow_mut().base.get_new_loading_text()
    }
}

fn strings_nocase_cmp(x: &str, y: &str) -> Ordering {
    let mut a = x.chars();
    let mut b = y.chars();

    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (Some(ca), Some(cb)) => {
                let mut cal = ca.to_lowercase();
                let mut cbl = cb.to_lowercase();

                loop {
                    match (cal.next(), cbl.next()) {
                        (None, None) => break,
                        (Some(_), None) => return Ordering::Greater,
                        (None, Some(_)) => return Ordering::Less,
                        (Some(calc), Some(cblc)) => match calc.cmp(&cblc) {
                            Ordering::Less => return Ordering::Less,
                            Ordering::Greater => return Ordering::Greater,
                            Ordering::Equal => {}
                        },
                    }
                }
            }
        }
    }
}

fn users_sort_cmp(x: (&String, UserRole), y: (&String, UserRole)) -> Ordering {
    match (x.1, y.1) {
        (UserRole::Admin, UserRole::Regular) => Ordering::Less,
        (UserRole::Regular, UserRole::Admin) => Ordering::Greater,
        _ => strings_nocase_cmp(x.0, y.0),
    }
}

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn did_list_change(&self) -> bool {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        let changed = inner.did_list_change;
        inner.did_list_change = false;
        changed
    }

    fn users_loaded(&self, mut list: Vec<(String, UserRole)>) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();

        list.sort_unstable_by(|x, y| users_sort_cmp((&x.0, x.1), (&y.0, y.1)));
        inner.users = list;

        inner.did_list_change = true;
        inner.base.set_is_loading(false);
    }

    fn user_changed(&self, notification_type: UserNotificationType, username: String, role: UserRole) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();

        let search_result = inner.users.iter_mut().enumerate().find(|(_, user)| user.0.eq(&username));

        let mut changed = true;
        match notification_type {
            UserNotificationType::Registered | UserNotificationType::Updated => match search_result {
                Some((index, user)) => {
                    if user.1 == role {
                        changed = false;
                    } else {
                        let (username, role) = inner.users.remove(index);

                        let index = match inner.users.binary_search_by(|x| users_sort_cmp((&x.0, x.1), (&username, role))) {
                            Ok(idx) => idx,
                            Err(idx) => idx,
                        };

                        inner.users.insert(index, (username, role));
                    }
                }
                None => {
                    let index = match inner.users.binary_search_by(|x| users_sort_cmp((&x.0, x.1), (&username, role))) {
                        Ok(idx) => idx,
                        Err(idx) => idx,
                    };

                    inner.users.insert(index, (username, role));
                }
            },
            UserNotificationType::Deleted => {
                if let Some((index, _)) = search_result {
                    inner.users.remove(index);
                }
            }
        };

        if changed {
            inner.did_list_change = true;
            inner.base.base.redraw_notify();
        }
    }

    fn on_type_filter_changed(&self, type_filter: UserTypeFilter) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        inner.type_filter = type_filter;
        inner.did_list_change = true;
        inner.base.base.redraw_notify();
    }

    fn on_username_filter_changed(&self, text: &str) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        inner.username_filter.clear();
        inner.username_filter.push_str(text.trim());
        inner.did_list_change = true;
        inner.base.base.redraw_notify();
    }

    fn filter_list_into(&self, destination: &mut Vec<(String, UserRole)>) {
        let inner_guard = self.inner.borrow();
        let inner = inner_guard.deref();

        let iter = inner.users.iter().filter(|(_, role)| match inner.type_filter {
            UserTypeFilter::All => true,
            UserTypeFilter::Regular => *role == UserRole::Regular,
            UserTypeFilter::Admin => *role == UserRole::Admin,
        });

        if inner.username_filter.is_empty() {
            for (username, role) in iter {
                destination.push((username.clone(), *role));
            }

            return;
        }

        let iter = iter.filter(|(username, _)| {
            for filter_word in inner.username_filter.split_whitespace() {
                let filter_word = filter_word.to_lowercase();
                let mut split = username.split_whitespace();
                if split.any(|username_word| username_word.to_lowercase().contains(&filter_word)) {
                    return true;
                }
            }

            false
        });

        for (username, role) in iter {
            destination.push((username.clone(), *role));
        }
    }
}

pub struct UsersPopup<W: AsyncWrite + Unpin + 'static> {
    base: LoadingPopup<Controller<W>, UsersPopupContent<W>>,
    background_task: JoinHandle<()>,
}

impl<W: AsyncWrite + Unpin + 'static> Drop for UsersPopup<W> {
    fn drop(&mut self) {
        self.background_task.abort();
    }
}

struct UsersPopupContent<W: AsyncWrite + Unpin + 'static> {
    type_filter: Padded<HorizontalSplit<CenteredTextLine, ArrowSelector<FilterArrowHandler<W>>>>,
    username_filter: Padded<HorizontalSplit<CenteredTextLine, TextEntry<UsernameEntryHandler<W>>>>,
    user_list: LongList<UserListHandler<W>>,
    help_text: Padded<CenteredText>,
    add_button: Padded<CenteredButton<AddButtonHandler<W>>>,
    user_list_height: u16,
    help_text_height: u16,
    focused_element: FocusedElement,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusedElement {
    TypeFilter,
    UsernameFilter,
    UserList,
    AddButton,
}

impl FocusedElement {
    fn down(self) -> Self {
        match self {
            Self::TypeFilter => Self::UsernameFilter,
            Self::UsernameFilter => Self::UserList,
            Self::UserList => Self::AddButton,
            Self::AddButton => Self::AddButton,
        }
    }

    fn up(self) -> Self {
        match self {
            Self::TypeFilter => Self::TypeFilter,
            Self::UsernameFilter => Self::TypeFilter,
            Self::UserList => Self::UsernameFilter,
            Self::AddButton => Self::UserList,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::TypeFilter => Self::UsernameFilter,
            Self::UsernameFilter => Self::UserList,
            Self::UserList => Self::AddButton,
            Self::AddButton => Self::TypeFilter,
        }
    }
}

struct FilterArrowHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> FilterArrowHandler<W> {
    fn new(controller: Rc<Controller<W>>) -> Self {
        Self { controller }
    }
}

impl<W: AsyncWrite + Unpin + 'static> ArrowSelectorHandler for FilterArrowHandler<W> {
    fn selection_changed(&mut self, selected_index: usize) {
        let type_filter = match selected_index {
            1 => UserTypeFilter::Regular,
            2 => UserTypeFilter::Admin,
            _ => UserTypeFilter::All,
        };

        self.controller.on_type_filter_changed(type_filter);
    }
}

struct UsernameEntryHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> UsernameEntryHandler<W> {
    fn new(controller: Rc<Controller<W>>) -> Self {
        Self { controller }
    }
}

impl<W: AsyncWrite + Unpin + 'static> TextEntryHandler for UsernameEntryHandler<W> {
    fn on_enter(&mut self, _controller: &Rc<TextEntryController>) -> OnEnterResult {
        OnEnterResult::Unhandled
    }

    fn on_char(&mut self, _controller: &Rc<TextEntryController>, _c: char, _cursor: &CursorPosition) -> bool {
        true
    }

    fn on_text_changed(&mut self, controller: &Rc<TextEntryController>) -> bool {
        controller.with_text(|text| self.controller.on_username_filter_changed(text));
        true
    }
}

impl<W: AsyncWrite + Unpin + 'static> UsersPopupContent<W> {
    fn new(redraw_notify: Rc<Notify>, controller: Rc<Controller<W>>) -> Self {
        let text_style = Style::new().fg(TEXT_COLOR);
        let selected_text_style = text_style.bg(SELECTED_BACKGROUND_COLOR);

        let type_filter_selector = ArrowSelector::new(
            Rc::clone(&redraw_notify),
            vec![
                (FILTER_ALL_STR.into(), FILTER_ALL_SHORTCUT),
                (FILTER_REGULAR_STR.into(), FILTER_REGULAR_SHORTCUT),
                (FILTER_ADMIN_STR.into(), FILTER_ADMIN_SHORTCUT),
            ],
            0,
            text_style,
            selected_text_style,
            selected_text_style,
            selected_text_style,
            false,
            FilterArrowHandler::new(Rc::clone(&controller)),
        );

        let type_filter_label = CenteredTextLine::new(TYPE_FILTER_LABEL.into(), text_style);
        let type_filter_inner = HorizontalSplit::new(type_filter_label, type_filter_selector, 0, 1);
        let type_filter = Padded::new(Padding::horizontal(1), type_filter_inner);

        let username_filter_entry = TextEntry::new(
            Rc::clone(&redraw_notify),
            String::new(),
            text_style,
            selected_text_style,
            USERNAME_FILTER_MAX_LENGTH,
            UsernameEntryHandler::new(Rc::clone(&controller)),
        );

        let username_filter_label = CenteredTextLine::new(USERNAME_FILTER_LABEL.into(), text_style);
        let username_filter_inner = HorizontalSplit::new(username_filter_label, username_filter_entry, 0, 1);
        let username_filter = Padded::new(Padding::horizontal(1), username_filter_inner);

        let user_list = LongList::new(
            Rc::clone(&redraw_notify),
            "".into(),
            0,
            false,
            true,
            UserListHandler::new(Rc::clone(&controller)),
        );

        let help_text_inner = CenteredText::new(HELP_MESSAGE.into(), HELP_MESSAGE_STYLE);
        let help_text = Padded::new(Padding::horizontal(1), help_text_inner);

        let add_button_inner = CenteredButton::new(
            redraw_notify,
            ADD_USER_BUTTON_TEXT.into(),
            text_style,
            selected_text_style,
            Some(ADD_USER_SHORTCUT_KEY),
            AddButtonHandler::new(Rc::clone(&controller)),
        );
        let add_button = Padded::new(Padding::uniform(1), add_button_inner);

        Self {
            type_filter,
            username_filter,
            user_list,
            help_text,
            add_button,
            user_list_height: 1,
            help_text_height: 1,
            focused_element: FocusedElement::TypeFilter,
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for UsersPopupContent<W> {
    fn resize(&mut self, area: Rect) {
        if area.is_empty() {
            return;
        }

        let mut remaining_area = area;

        let mut type_filter_area = remaining_area;
        type_filter_area.height = 1;
        self.type_filter.resize(type_filter_area);

        remaining_area.height -= 1;
        remaining_area.y += 1;
        if remaining_area.height == 0 {
            return;
        }

        let mut username_filter_area = remaining_area;
        username_filter_area.height = 1;
        self.username_filter.resize(username_filter_area);

        remaining_area.height -= 1;
        remaining_area.y += 1;
        if remaining_area.height < 3 {
            return;
        }

        let mut add_button_area = remaining_area;
        add_button_area.y = add_button_area.bottom() - 3;
        add_button_area.height = 3;
        self.add_button.resize(add_button_area);

        remaining_area.height -= 3;
        if remaining_area.height == 0 {
            return;
        }

        let help_text_size = self.help_text.begin_resize(remaining_area.width, remaining_area.height);
        let mut help_text_area = remaining_area;
        help_text_area.height = help_text_area.height.min(help_text_size.1);
        help_text_area.y += remaining_area.height - help_text_area.height;
        self.help_text.resize(help_text_area);
        self.help_text_height = help_text_area.height;

        remaining_area.height -= help_text_area.height;
        if remaining_area.height < 2 {
            return;
        }

        self.user_list.resize(remaining_area);
        self.user_list_height = remaining_area.height;
    }

    fn render(&mut self, mut area: Rect, frame: &mut Frame) {
        if area.is_empty() {
            return;
        }

        let controller = &self.user_list.handler.controller;
        if controller.did_list_change() {
            let list = &mut self.user_list.handler.users_filtered;
            list.clear();
            controller.filter_list_into(list);
            let list_len = list.len();
            self.user_list.reset_items_no_redraw(list_len, true);
        }

        let type_filter_area = Rect::new(area.x, area.y, area.width, 1);
        self.type_filter.render(type_filter_area, frame);
        area.y += 1;
        area.height -= 1;
        if area.height == 0 {
            return;
        }

        let username_filter_area = Rect::new(area.x, area.y, area.width, 1);
        self.username_filter.render(username_filter_area, frame);
        area.y += 1;
        area.height -= 1;
        if area.height < self.user_list_height {
            return;
        }

        let user_list_area = Rect::new(area.x, area.y, area.width, self.user_list_height);
        self.user_list.render(user_list_area, frame);
        area.y += self.user_list_height;
        area.height -= self.user_list_height;
        if area.height < self.help_text_height {
            return;
        }

        let help_text_area = Rect::new(area.x, area.y, area.width, self.help_text_height);
        self.help_text.render(help_text_area, frame);
        area.y += self.help_text_height;
        area.height -= self.help_text_height;
        if area.height == 0 {
            return;
        }

        self.add_button.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        if is_focused {
            let status = match self.focused_element {
                FocusedElement::TypeFilter => self.type_filter.handle_event(event, true),
                FocusedElement::UsernameFilter => self.username_filter.handle_event(event, true),
                FocusedElement::UserList => self.user_list.handle_event(event, true),
                FocusedElement::AddButton => self.add_button.handle_event(event, true),
            };

            match status {
                HandleEventStatus::Handled => return HandleEventStatus::Handled,
                HandleEventStatus::PassFocus(focus_position, direction) => {
                    let next_focused_element = match direction {
                        PassFocusDirection::Up => self.focused_element.up(),
                        PassFocusDirection::Down => self.focused_element.down(),
                        PassFocusDirection::Forward => self.focused_element.next(),
                        _ => return status,
                    };

                    if next_focused_element == self.focused_element {
                        return status;
                    }

                    let focus_passed = match next_focused_element {
                        FocusedElement::TypeFilter => self.type_filter.receive_focus(focus_position),
                        FocusedElement::UsernameFilter => self.username_filter.receive_focus(focus_position),
                        FocusedElement::UserList => self.user_list.receive_focus(focus_position),
                        FocusedElement::AddButton => self.add_button.receive_focus(focus_position),
                    };

                    if !focus_passed {
                        return status;
                    }

                    match self.focused_element {
                        FocusedElement::TypeFilter => self.type_filter.focus_lost(),
                        FocusedElement::UsernameFilter => self.username_filter.focus_lost(),
                        FocusedElement::UserList => self.user_list.focus_lost(),
                        FocusedElement::AddButton => self.add_button.focus_lost(),
                    }

                    self.focused_element = next_focused_element;
                    return HandleEventStatus::Handled;
                }
                HandleEventStatus::Unhandled => {}
            }
        }

        self.type_filter
            .handle_event(event, false)
            .or_else(|| self.username_filter.handle_event(event, false))
            .or_else(|| self.user_list.handle_event(event, false))
            .or_else(|| self.add_button.handle_event(event, false))
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.focused_element = FocusedElement::TypeFilter;
        self.type_filter.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        match self.focused_element {
            FocusedElement::TypeFilter => self.type_filter.focus_lost(),
            FocusedElement::UsernameFilter => self.username_filter.focus_lost(),
            FocusedElement::UserList => self.user_list.focus_lost(),
            FocusedElement::AddButton => self.add_button.focus_lost(),
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> AutosizeUIElement for UsersPopupContent<W> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        let (_, help_text_height) = self.help_text.begin_resize(width, height);
        let mut request_height = help_text_height;

        let list_len = self.user_list.handler.controller.inner.borrow().users.len();
        let user_list_height = list_len.max(MIN_USER_LIST_HEIGHT).saturating_add(2).min(u16::MAX as usize) as u16;
        request_height = request_height.saturating_add(user_list_height);

        self.type_filter.begin_resize(width, 1);
        self.username_filter.begin_resize(width, 1);
        let type_filter_height = 1;
        let username_filter_height = 1;
        let add_button_height = 3;
        let additional_height = type_filter_height + username_filter_height + add_button_height;
        request_height = request_height.saturating_add(additional_height);

        (width, request_height.min(height))
    }
}

struct UserListHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
    users_filtered: Vec<(String, UserRole)>,
}

impl<W: AsyncWrite + Unpin + 'static> UserListHandler<W> {
    fn new(controller: Rc<Controller<W>>) -> Self {
        Self {
            controller,
            users_filtered: Vec::new(),
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> LongListHandler for UserListHandler<W> {
    fn get_item_lines<F: FnMut(Line<'static>)>(&mut self, index: usize, wrap_width: u16, f: F) {
        let user = &self.users_filtered[index];
        // TODO: Display prettier
        let text = format!(" [{}] {}", user.1.into_role_char(), &user.0);
        let iter = [(StaticString::Owned(text), Style::new().fg(TEXT_COLOR))].into_iter();
        wrap_lines_by_chars(wrap_width as usize, iter, f);
    }

    fn modify_line_to_selected(&mut self, _index: usize, line: &mut Line<'static>, _item_line_number: u16) {
        for span in line.spans.iter_mut() {
            span.style.bg = Some(SELECTED_BACKGROUND_COLOR);
        }
    }

    fn modify_line_to_unselected(&mut self, _index: usize, line: &mut Line<'static>, _item_line_number: u16) {
        for span in line.spans.iter_mut() {
            span.style.bg = None;
        }
    }

    fn on_enter(&mut self, _index: usize) -> OnEnterResult {
        // TODO: Open the update/delete popup
        /*let inner_guard = self.controller.inner.borrow();
        let inner = inner_guard.deref();

        let popup = CloseSocketPopup::new(
            Rc::clone(&inner.base.base.redraw_notify),
            Weak::clone(&self.controller.manager),
            self.users_filtered[index],
            self.controller.socket_type,
            self.controller.sockets_watch.clone(),
            self.controller.popup_sender.clone(),
        );

        let _ = self.controller.popup_sender.send(popup.into());*/

        OnEnterResult::Handled
    }
}

struct AddButtonHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> AddButtonHandler<W> {
    fn new(controller: Rc<Controller<W>>) -> Self {
        Self { controller }
    }
}

impl<W: AsyncWrite + Unpin + 'static> ButtonHandler for AddButtonHandler<W> {
    fn on_pressed(&mut self) -> OnEnterResult {
        // TODO: Open the new user popup
        /*let inner_guard = self.controller.inner.borrow();
        let inner = inner_guard.deref();

        let popup = AddSocketPopup::new(
            Rc::clone(&inner.base.base.redraw_notify),
            Weak::clone(&self.controller.manager),
            self.controller.socket_type,
            self.controller.sockets_watch.clone(),
            self.controller.popup_sender.clone(),
        );

        let _ = self.controller.popup_sender.send(popup.into());*/

        OnEnterResult::Handled
    }
}

async fn load_users_task<W: AsyncWrite + Unpin + 'static>(controller_weak: Weak<Controller<W>>) {
    let manager_rc = match controller_weak.upgrade().map(|rc| rc.manager.upgrade()) {
        Some(Some(manager_rc)) => manager_rc,
        _ => return,
    };

    let (response_sender, response_receiver) = oneshot::channel();
    let send_status = manager_rc
        .list_users_fn(|result| {
            let _ = response_sender.send(result.0);
        })
        .await;
    drop(manager_rc);

    let maybe_list = if send_status.is_err() {
        None
    } else {
        match response_receiver.await {
            Ok(list) => Some(list),
            Err(_) => None,
        }
    };

    let rc = match controller_weak.upgrade() {
        Some(rc) => rc,
        None => return,
    };

    let mut update_watch = match maybe_list {
        Some(list) => {
            rc.users_loaded(list);
            rc.users_watch.subscribe()
        }
        None => {
            rc.set_new_loading_text(REQUEST_SEND_ERROR_MESSAGE.into());
            return;
        }
    };

    loop {
        let (notification_type, username, role) = match update_watch.recv().await {
            Ok(t) => t,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return,
        };

        let rc = match controller_weak.upgrade() {
            Some(rc) => rc,
            None => return,
        };

        rc.user_changed(notification_type, username, role);
    }
}

impl<W: AsyncWrite + Unpin + 'static> UsersPopup<W> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        users_watch: broadcast::Sender<(UserNotificationType, String, UserRole)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = Controller::new(Rc::clone(&redraw_notify), manager, users_watch, popup_sender);
        let controller = Rc::new(controller);

        let controller_weak = Rc::downgrade(&controller);

        let content = UsersPopupContent::new(redraw_notify, Rc::clone(&controller));

        let base = LoadingPopup::new(
            TITLE.into(),
            LOADING_MESSAGE.into(),
            LOADING_STYLE,
            TEXT_COLOR,
            BACKGROUND_COLOR,
            SizeConstraint::new().max(POPUP_WIDTH, MAX_POPUP_HEIGHT),
            controller,
            content,
        );

        let background_task = tokio::task::spawn_local(async move {
            load_users_task(controller_weak).await;
        });

        let value = Self { base, background_task };
        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for UsersPopup<W> {
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
