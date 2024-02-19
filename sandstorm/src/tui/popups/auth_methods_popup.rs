use std::{
    borrow::BorrowMut,
    cell::RefCell,
    ops::DerefMut,
    rc::{Rc, Weak},
};

use crossterm::event;
use dust_devil_core::socks5::AuthMethod;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
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
            long_list::{LongList, LongListHandler},
            text::Text,
            vertical_split::VerticalSplit,
            OnEnterResult,
        },
        text_wrapper::{StaticString, WrapTextIter},
        ui_element::{AutosizeUIElement, HandleEventStatus, UIElement},
        ui_manager::Popup,
    },
};

use super::{
    loading_popup::{LoadingPopup, LoadingPopupController, LoadingPopupControllerInner},
    message_popup::{MessagePopup, ERROR_POPUP_TITLE, REQUEST_SEND_ERROR_MESSAGE},
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
};

const TEXT_COLOR: Color = Color::Black;
const BACKGROUND_COLOR: Color = Color::Cyan;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightCyan;
const LOCKED_BACKGROUND_COLOR: Color = Color::LightBlue;

const ENABLED_STATUS_COLOR: Color = Color::Green;
const SELECTED_ENABLED_STATUS_COLOR: Color = Color::LightGreen;

const DISABLED_STATUS_COLOR: Color = Color::Red;
const SELECTED_DISABLED_STATUS_COLOR: Color = Color::LightRed;

const TITLE: &str = "─Authentication Methods";
const LOADING_MESSAGE: &str = "Getting authentication methods from the server...";
const TOP_MESSAGE: &str = "The server supports the following authentication methods:";
const HELP_MESSAGE: &str = "Scroll the list with the arrow keys, press (ENTER) on an auth method to toggle it.";
const POPUP_WIDTH: u16 = 34;
const MAX_POPUP_HEIGHT: u16 = 24;

const SERVER_ERROR_MESSAGE: &str = "The server refused to toggle this authentication method.";
const ERROR_POPUP_WIDTH: u16 = 40;

struct ControllerInner {
    base: LoadingPopupControllerInner,
    auth_methods: Vec<(AuthMethod, bool)>,
    did_list_change: bool,
    is_setting_auth_method: bool,
    set_auth_method_task: Option<JoinHandle<()>>,
}

impl Drop for ControllerInner {
    fn drop(&mut self) {
        if let Some(handle) = self.set_auth_method_task.take() {
            handle.abort();
        }
    }
}

struct Controller<W: AsyncWrite + Unpin + 'static> {
    inner: RefCell<ControllerInner>,
    manager: Weak<MutexedSandstormRequestManager<W>>,
    auth_methods_watch: broadcast::Sender<(AuthMethod, bool)>,
    popup_sender: mpsc::UnboundedSender<Popup>,
}

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        auth_methods_watch: broadcast::Sender<(AuthMethod, bool)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (base, close_receiver) = LoadingPopupControllerInner::new(redraw_notify, true, true);

        let inner = ControllerInner {
            base,
            auth_methods: Vec::new(),
            did_list_change: false,
            is_setting_auth_method: false,
            set_auth_method_task: None,
        };

        let value = Self {
            inner: RefCell::new(inner),
            manager,
            auth_methods_watch,
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

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn did_list_change(&self) -> bool {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        let changed = inner.did_list_change;
        inner.did_list_change = false;
        changed
    }

    fn auth_methods_loaded(&self, list: Vec<(AuthMethod, bool)>) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.borrow_mut();
        inner.auth_methods = list;
        inner.did_list_change = true;
        inner.base.set_is_loading(false);
    }

    fn auth_method_changed(&self, auth_method: AuthMethod, state: bool) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.borrow_mut();
        inner.did_list_change = true;

        match inner.auth_methods.iter_mut().find(|(m, _)| *m == auth_method) {
            Some((_, saved_state)) => {
                *saved_state = state;
                inner.base.base.redraw_notify();
            }
            None => {
                inner.auth_methods.push((auth_method, state));
                inner.base.base.request_resize();
            }
        }
    }

    fn set_auth_method(self: &Rc<Self>, auth_method: AuthMethod, state: bool) {
        let mut inner = self.inner.borrow_mut();
        if inner.is_setting_auth_method {
            return;
        }

        inner.is_setting_auth_method = true;
        inner.did_list_change = true;
        inner.base.base.set_closable(false);
        drop(inner);

        let self_weak = Rc::downgrade(self);
        let handle = tokio::task::spawn_local(async move {
            set_auth_method_task(self_weak, auth_method, state).await;
        });

        self.inner.borrow_mut().set_auth_method_task = Some(handle);
    }
}

async fn set_auth_method_task<W: AsyncWrite + Unpin + 'static>(
    controller_weak: Weak<Controller<W>>,
    auth_method: AuthMethod,
    status: bool,
) {
    let manager_rc = match controller_weak.upgrade().map(|rc| rc.manager.upgrade()) {
        Some(Some(rc)) => rc,
        _ => return,
    };

    let (response_sender, response_receiver) = oneshot::channel();
    let send_status = manager_rc
        .toggle_auth_method_fn(auth_method, status, |result| {
            let _ = response_sender.send(result.0);
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

    let rc = match controller_weak.upgrade() {
        Some(rc) => rc,
        None => return,
    };

    match maybe_result {
        Some(true) => {
            let _ = rc.auth_methods_watch.send((auth_method, status));
        }
        Some(false) | None => {
            let error_str = match maybe_result {
                None => REQUEST_SEND_ERROR_MESSAGE,
                _ => SERVER_ERROR_MESSAGE,
            };

            let popup = MessagePopup::empty_error_message(
                Rc::clone(&rc.inner.borrow().base.base.redraw_notify),
                ERROR_POPUP_TITLE.into(),
                error_str.into(),
                ERROR_POPUP_WIDTH,
            );

            let _ = rc.popup_sender.send(popup.into());
        }
    }

    let mut inner = rc.inner.borrow_mut();
    inner.is_setting_auth_method = false;
    inner.did_list_change = true;
    inner.base.base.set_closable(true);
}

pub struct AuthMethodsPopup<W: AsyncWrite + Unpin + 'static> {
    base: LoadingPopup<Controller<W>, AuthMethodsContent<W>>,
    background_task: JoinHandle<()>,
}

impl<W: AsyncWrite + Unpin + 'static> Drop for AuthMethodsPopup<W> {
    fn drop(&mut self) {
        self.background_task.abort();
    }
}

async fn load_methods_task<W: AsyncWrite + Unpin + 'static>(controller_weak: Weak<Controller<W>>) {
    let manager_rc = match controller_weak.upgrade().map(|rc| rc.manager.upgrade()) {
        Some(Some(rc)) => rc,
        _ => return,
    };

    let (response_sender, response_receiver) = oneshot::channel();
    let send_status = manager_rc
        .list_auth_methods_fn(|result| {
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
            rc.auth_methods_loaded(list);
            rc.auth_methods_watch.subscribe()
        }
        None => {
            rc.set_new_loading_text(REQUEST_SEND_ERROR_MESSAGE.into());
            return;
        }
    };

    loop {
        let (auth_method, state) = match update_watch.recv().await {
            Ok(t) => t,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return,
        };

        let rc = match controller_weak.upgrade() {
            Some(rc) => rc,
            None => return,
        };

        rc.auth_method_changed(auth_method, state);
    }
}

impl<W: AsyncWrite + Unpin + 'static> AuthMethodsPopup<W> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        auth_methods_watch: broadcast::Sender<(AuthMethod, bool)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = Controller::new(Rc::clone(&redraw_notify), manager, auth_methods_watch, popup_sender);
        let controller = Rc::new(controller);

        let content = AuthMethodsContent::new(redraw_notify, Rc::clone(&controller));
        let controller_weak = Rc::downgrade(&controller);

        let base = LoadingPopup::new(
            TITLE.into(),
            LOADING_MESSAGE.into(),
            Style::new().fg(TEXT_COLOR),
            TEXT_COLOR,
            BACKGROUND_COLOR,
            SizeConstraint::new().max(POPUP_WIDTH, MAX_POPUP_HEIGHT),
            controller,
            content,
        );

        let background_task = tokio::task::spawn_local(async move {
            load_methods_task(controller_weak).await;
        });

        let value = Self { base, background_task };
        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for AuthMethodsPopup<W> {
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

struct AuthMethodsContent<W: AsyncWrite + Unpin + 'static> {
    base: VerticalSplit<Text, VerticalSplit<LongList<AuthListHandler<W>>, Text>>,
}

impl<W: AsyncWrite + Unpin + 'static> AuthMethodsContent<W> {
    fn new(redraw_notify: Rc<Notify>, controller: Rc<Controller<W>>) -> Self {
        let text_color = Style::new().fg(TEXT_COLOR);
        let top_text = Text::new(TOP_MESSAGE.into(), text_color, Alignment::Center);
        let bottom_text = Text::new(HELP_MESSAGE.into(), text_color, Alignment::Center);

        let list = LongList::new(redraw_notify, "".into(), 0, false, true, AuthListHandler { controller });

        let lower_split = VerticalSplit::new(list, bottom_text, 0, 0);

        Self {
            base: VerticalSplit::new(top_text, lower_split, 0, 0),
        }
    }
}

struct AuthListHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> LongListHandler for AuthListHandler<W> {
    fn get_item_lines<F: FnMut(Line<'static>)>(&mut self, index: usize, wrap_width: u16, mut f: F) {
        let wrap_width = wrap_width.max(20) as usize;

        let inner = self.controller.inner.borrow();
        let (auth_method, enabled) = inner.auth_methods[index];

        let auth_method_str = match auth_method {
            AuthMethod::NoAuth => "No authentication",
            AuthMethod::UsernameAndPassword => "Username and password",
        };

        let (status_color, status_str, background_color) = if inner.is_setting_auth_method {
            let (status_color, status_str) = match enabled {
                true => (SELECTED_ENABLED_STATUS_COLOR, " [Y] "),
                false => (SELECTED_DISABLED_STATUS_COLOR, " [N] "),
            };

            (status_color, status_str, LOCKED_BACKGROUND_COLOR)
        } else {
            let (status_color, status_str) = match enabled {
                true => (ENABLED_STATUS_COLOR, " [Y] "),
                false => (DISABLED_STATUS_COLOR, " [N] "),
            };

            (status_color, status_str, BACKGROUND_COLOR)
        };

        let text_style = Style::new().fg(TEXT_COLOR).bg(background_color);

        let mut iter = WrapTextIter::new(auth_method_str, wrap_width - 5);

        let auth_line_range = iter.next().unwrap();
        let spans = vec![
            Span::styled(status_str, text_style.bg(status_color)),
            Span::styled(auth_line_range.get_substr(auth_method_str), text_style),
        ];
        f(Line::from(spans));

        for auth_line_range in iter {
            let spans = vec![
                Span::styled("     ", text_style.bg(status_color)),
                Span::styled(auth_line_range.get_substr(auth_method_str), text_style),
            ];
            f(Line::from(spans));
        }
    }

    fn modify_line_to_selected(&mut self, _index: usize, line: &mut Line<'static>, _item_line_number: u16) {
        for span in line.spans.iter_mut() {
            span.style.bg = span.style.bg.map(|color| match color {
                BACKGROUND_COLOR => SELECTED_BACKGROUND_COLOR,
                ENABLED_STATUS_COLOR => SELECTED_ENABLED_STATUS_COLOR,
                DISABLED_STATUS_COLOR => SELECTED_DISABLED_STATUS_COLOR,
                other => other,
            });
        }
    }

    fn modify_line_to_unselected(&mut self, _index: usize, line: &mut Line<'static>, _item_line_number: u16) {
        for span in line.spans.iter_mut() {
            span.style.bg = span.style.bg.map(|color| match color {
                SELECTED_BACKGROUND_COLOR => BACKGROUND_COLOR,
                SELECTED_ENABLED_STATUS_COLOR => ENABLED_STATUS_COLOR,
                SELECTED_DISABLED_STATUS_COLOR => DISABLED_STATUS_COLOR,
                other => other,
            });
        }
    }

    fn on_enter(&mut self, index: usize) -> OnEnterResult {
        let (auth_method, enabled) = self.controller.inner.borrow().auth_methods[index];
        self.controller.set_auth_method(auth_method, !enabled);
        OnEnterResult::Handled
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for AuthMethodsContent<W> {
    fn resize(&mut self, area: Rect) {
        self.base.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let controller = &self.base.lower.upper.handler.controller;
        if controller.did_list_change() {
            self.base
                .lower
                .upper
                .reset_items_no_redraw(controller.inner.borrow().auth_methods.len(), true);
        }

        self.base.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        if self.base.lower.upper.handler.controller.inner.borrow().is_setting_auth_method {
            return HandleEventStatus::Handled;
        }

        self.base.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.base.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.base.focus_lost();
    }
}

impl<W: AsyncWrite + Unpin + 'static> AutosizeUIElement for AuthMethodsContent<W> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        let controller = &self.base.lower.upper.handler.controller;
        if controller.did_list_change() {
            self.base
                .lower
                .upper
                .reset_items_no_redraw(controller.inner.borrow().auth_methods.len(), true);
        }

        self.base.begin_resize(width, height)
    }
}
