use std::{
    cell::RefCell,
    cmp::Ordering,
    io::Error,
    net::SocketAddr,
    ops::{Deref, DerefMut},
    rc::{Rc, Weak},
};

use crossterm::event;
use dust_devil_core::sandstorm::RemoveSocketResponse;
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
            centered_text::CenteredText,
            long_list::{LongList, LongListHandler},
            padded::Padded,
            vertical_split::VerticalSplit,
            OnEnterResult,
        },
        text_wrapper::StaticString,
        ui_element::{AutosizeUIElement, HandleEventStatus, UIElement},
        ui_manager::Popup,
    },
};

use super::{
    loading_popup::{LoadingPopup, LoadingPopupController, LoadingPopupControllerInner},
    message_popup::REQUEST_SEND_ERROR_MESSAGE,
    popup_base::PopupBaseController,
    size_constraint::{ConstrainedPopupContent, SizeConstraint},
};

const BACKGROUND_COLOR: Color = Color::Yellow;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightYellow;

const SOCKS5_TITLE: &str = "─Socks5 Sockets";
const SANDSTORM_TITLE: &str = "─Sandstorm Sockets";
const LOADING_MESSAGE: &str = "Getting socket list from the server...";
const LOADING_STYLE: Style = Style::new();
const SOCKS5_TOP_MESSAGE: &str = "Listening socks5 sockets:";
const SANDSTORM_TOP_MESSAGE: &str = "Listening Sandstorm sockets:";
const TOP_MESSAGE_STYLE: Style = Style::new();
const HELP_MESSAGE: &str = "Scroll the list with the arrow keys, press (ENTER) on a socket to close it.";
const HELP_MESSAGE_STYLE: Style = Style::new();
const ADD_SOCKET_BUTTON_TEXT: &str = "[add new socket (a)]";
const ADD_SOCKET_SHORTCUT_KEY: char = 'a';
const POPUP_WIDTH: u16 = 40;
const MAX_POPUP_HEIGHT: u16 = 24;

const SERVER_ADD_ERROR_MESSAGE: &str = "The server encountered an error while opening the socket:";
const SERVER_REMOVE_ERROR_MESSAGE: &str = "The server could not find the socket you requested to remove.";
const ERROR_POPUP_WIDTH: u16 = 40;

const FILTER_ALL_STR: &str = "[ALL]";
const FILTER_IPV4_STR: &str = "[IPv4]";
const FILTER_IPV6_STR: &str = "[IPv6]";

#[derive(Clone, Copy)]
pub enum SocketPopupType {
    Socks5,
    Sandstorm,
}

impl SocketPopupType {
    async fn list_sockets<W: AsyncWrite + Unpin + 'static>(
        self,
        manager: Rc<MutexedSandstormRequestManager<W>>,
    ) -> Result<Vec<SocketAddr>, bool> {
        let (response_sender, response_receiver) = oneshot::channel();
        let send_status = match self {
            SocketPopupType::Socks5 => {
                manager
                    .list_socks5_sockets_fn(|result| {
                        let _ = response_sender.send(result.0);
                    })
                    .await
            }
            SocketPopupType::Sandstorm => {
                manager
                    .list_sandstorm_sockets_fn(|result| {
                        let _ = response_sender.send(result.0);
                    })
                    .await
            }
        };
        drop(manager);

        send_status.map_err(|_| false)?;
        response_receiver.await.map_err(|_| true)
    }

    async fn add_socket<W: AsyncWrite + Unpin + 'static>(
        self,
        manager: Rc<MutexedSandstormRequestManager<W>>,
        socket_address: SocketAddr,
    ) -> Result<Result<(), Error>, bool> {
        let (response_sender, response_receiver) = oneshot::channel();
        let send_status = match self {
            SocketPopupType::Socks5 => {
                manager
                    .add_socks5_socket_fn(socket_address, |result| {
                        let _ = response_sender.send(result.0);
                    })
                    .await
            }
            SocketPopupType::Sandstorm => {
                manager
                    .add_sandstorm_socket_fn(socket_address, |result| {
                        let _ = response_sender.send(result.0);
                    })
                    .await
            }
        };
        drop(manager);

        send_status.map_err(|_| false)?;
        response_receiver.await.map_err(|_| true)
    }

    async fn remove_socket<W: AsyncWrite + Unpin + 'static>(
        self,
        manager: Rc<MutexedSandstormRequestManager<W>>,
        socket_address: SocketAddr,
    ) -> Result<RemoveSocketResponse, bool> {
        let (response_sender, response_receiver) = oneshot::channel();
        let send_status = match self {
            SocketPopupType::Socks5 => {
                manager
                    .remove_socks5_socket_fn(socket_address, |result| {
                        let _ = response_sender.send(result.0);
                    })
                    .await
            }
            SocketPopupType::Sandstorm => {
                manager
                    .remove_sandstorm_socket_fn(socket_address, |result| {
                        let _ = response_sender.send(result.0);
                    })
                    .await
            }
        };
        drop(manager);

        send_status.map_err(|_| false)?;
        response_receiver.await.map_err(|_| true)
    }
}

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum IpFilterType {
    All = 0,
    IPv4 = 1,
    IPv6 = 2,
}

struct ControllerInner {
    base: LoadingPopupControllerInner,
    sockets: Vec<SocketAddr>,
    did_list_change: bool,
    ip_filter: IpFilterType,
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
        let (base, close_receiver) = LoadingPopupControllerInner::new(redraw_notify, true, true);

        let inner = ControllerInner {
            base,
            sockets: Vec::new(),
            did_list_change: false,
            ip_filter: IpFilterType::All,
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

fn socket_sort_cmp(x: &SocketAddr, y: &SocketAddr) -> Ordering {
    match (x, y) {
        (SocketAddr::V4(_), SocketAddr::V6(_)) => Ordering::Greater,
        (SocketAddr::V6(_), SocketAddr::V4(_)) => Ordering::Less,
        (SocketAddr::V4(x), SocketAddr::V4(y)) => x.port().cmp(&y.port()).then_with(|| x.ip().cmp(y.ip())),
        (SocketAddr::V6(x), SocketAddr::V6(y)) => x.port().cmp(&y.port()).then_with(|| x.ip().cmp(y.ip())),
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

    fn sockets_loaded(&self, mut list: Vec<SocketAddr>) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();

        list.sort_unstable_by(socket_sort_cmp);
        inner.sockets = list;

        inner.did_list_change = true;
        inner.base.set_is_loading(false);
    }

    fn socket_changed(&self, socket_address: SocketAddr, state: bool) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();

        if state {
            match inner.sockets.binary_search_by(|x| socket_sort_cmp(x, &socket_address)) {
                Ok(_) => return,
                Err(index) => inner.sockets.insert(index, socket_address),
            }
        } else {
            match inner.sockets.binary_search_by(|x| socket_sort_cmp(x, &socket_address)) {
                Ok(index) => inner.sockets.remove(index),
                Err(_) => return,
            };
        }

        inner.did_list_change = true;
        inner.base.base.redraw_notify();
    }

    fn on_ip_filter_changed(&self, ip_filter: IpFilterType) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        inner.ip_filter = ip_filter;
        inner.did_list_change = true;
        inner.base.base.redraw_notify();
    }

    fn filter_list_into(&self, destination: &mut Vec<SocketAddr>) {
        let inner_guard = self.inner.borrow();
        let inner = inner_guard.deref();

        let iter = inner.sockets.iter().filter(|socket_address| match inner.ip_filter {
            IpFilterType::All => true,
            IpFilterType::IPv4 => socket_address.is_ipv4(),
            IpFilterType::IPv6 => socket_address.is_ipv6(),
        });

        for socket_address in iter {
            destination.push(*socket_address);
        }
    }
}

pub struct SocketsPopup<W: AsyncWrite + Unpin + 'static> {
    base: LoadingPopup<Controller<W>, SocketPopupContent<W>>,
    background_task: JoinHandle<()>,
}

impl<W: AsyncWrite + Unpin + 'static> Drop for SocketsPopup<W> {
    fn drop(&mut self) {
        self.background_task.abort();
    }
}

type SocketPopupContent<W> = VerticalSplit<CenteredText, VerticalSplit<UpperContent<W>, BottomContent<W>>>;

type UpperContent<W> = VerticalSplit<Padded<ArrowSelector<FilterArrowHandler<W>>>, SocketList<W>>;

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
        let ip_filter = match selected_index {
            1 => IpFilterType::IPv4,
            2 => IpFilterType::IPv6,
            _ => IpFilterType::All,
        };

        self.controller.on_ip_filter_changed(ip_filter);
    }
}

struct SocketList<W: AsyncWrite + Unpin + 'static> {
    list: ConstrainedPopupContent<LongList<SocketListHandler<W>>>,
}

impl<W: AsyncWrite + Unpin + 'static> SocketList<W> {
    fn new(redraw_notify: Rc<Notify>, controller: Rc<Controller<W>>) -> Self {
        let list = LongList::new(redraw_notify, "".into(), 0, false, true, SocketListHandler::new(controller));
        let list = ConstrainedPopupContent::new(SizeConstraint::new().min(40, 8), list);
        Self { list }
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for SocketList<W> {
    fn resize(&mut self, area: Rect) {
        self.list.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let controller = &self.list.inner.handler.controller;
        if controller.did_list_change() {
            let list = &mut self.list.inner.handler.sockets_filtered;
            list.clear();
            controller.filter_list_into(list);
            let list_len = list.len();
            self.list.inner.reset_items_no_redraw(list_len, true);
        }

        self.list.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.list.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.list.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.list.focus_lost();
    }
}

impl<W: AsyncWrite + Unpin + 'static> AutosizeUIElement for SocketList<W> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        self.list.begin_resize(width, height)
    }
}

struct SocketListHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
    sockets_filtered: Vec<SocketAddr>,
}

impl<W: AsyncWrite + Unpin + 'static> SocketListHandler<W> {
    fn new(controller: Rc<Controller<W>>) -> Self {
        Self {
            controller,
            sockets_filtered: Vec::new(),
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> LongListHandler for SocketListHandler<W> {
    fn get_item_lines<F: FnMut(Line<'static>)>(&mut self, index: usize, wrap_width: u16, mut f: F) {
        let socket_address = self.sockets_filtered[index];
        f(Line::from(socket_address.to_string()))
    }

    fn modify_line_to_selected(&mut self, index: usize, line: &mut Line<'static>, item_line_number: u16) {
        for span in line.spans.iter_mut() {
            span.style.bg = Some(SELECTED_BACKGROUND_COLOR);
        }
    }

    fn modify_line_to_unselected(&mut self, index: usize, line: &mut Line<'static>, item_line_number: u16) {
        for span in line.spans.iter_mut() {
            span.style.bg = None;
        }
    }

    fn on_enter(&mut self, index: usize) -> OnEnterResult {
        OnEnterResult::Handled
    }
}

type BottomContent<W> = Padded<VerticalSplit<CenteredText, CenteredButton<AddButtonHandler<W>>>>;

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
        OnEnterResult::PassFocusAway
    }
}

async fn load_sockets_task<W: AsyncWrite + Unpin + 'static>(controller_weak: Weak<Controller<W>>) {
    let (manager_rc, socket_type) = match controller_weak.upgrade().map(|rc| (rc.manager.upgrade(), rc)) {
        Some((Some(manager_rc), rc)) => (manager_rc, rc.socket_type),
        _ => return,
    };

    let maybe_list = socket_type.list_sockets(manager_rc).await;

    let rc = match controller_weak.upgrade() {
        Some(rc) => rc,
        None => return,
    };

    let mut update_watch = match maybe_list {
        Ok(list) => {
            rc.sockets_loaded(list);
            rc.sockets_watch.clone().subscribe()
        }
        Err(_) => {
            rc.set_new_loading_text(REQUEST_SEND_ERROR_MESSAGE.into());
            return;
        }
    };

    loop {
        let (socket_address, state) = match update_watch.recv().await {
            Ok(t) => t,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return,
        };

        let rc = match controller_weak.upgrade() {
            Some(rc) => rc,
            None => return,
        };

        rc.socket_changed(socket_address, state);
    }
}

impl<W: AsyncWrite + Unpin + 'static> SocketsPopup<W> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        socket_type: SocketPopupType,
        sockets_watch: broadcast::Sender<(SocketAddr, bool)>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = Controller::new(Rc::clone(&redraw_notify), manager, socket_type, sockets_watch, popup_sender);
        let controller = Rc::new(controller);

        let ip_filter = ArrowSelector::new(
            Rc::clone(&redraw_notify),
            vec![
                (FILTER_ALL_STR.into(), Some('0')),
                (FILTER_IPV4_STR.into(), Some('4')),
                (FILTER_IPV6_STR.into(), Some('6')),
            ],
            0,
            Style::new(),
            Style::new().bg(SELECTED_BACKGROUND_COLOR),
            Style::new().bg(SELECTED_BACKGROUND_COLOR),
            Style::new().bg(SELECTED_BACKGROUND_COLOR),
            Style::new().bg(SELECTED_BACKGROUND_COLOR),
            false,
            FilterArrowHandler::new(Rc::clone(&controller)),
        );
        let ip_filter = Padded::new(Padding::horizontal(1), ip_filter);
        let socket_list = SocketList::new(Rc::clone(&redraw_notify), Rc::clone(&controller));

        let upper_content = VerticalSplit::new(ip_filter, socket_list, 0, 0);

        let help_text = CenteredText::new(HELP_MESSAGE.into(), HELP_MESSAGE_STYLE);
        let add_button = CenteredButton::new(
            Rc::clone(&redraw_notify),
            ADD_SOCKET_BUTTON_TEXT.into(),
            Style::new(),
            Style::new().bg(SELECTED_BACKGROUND_COLOR),
            Some(ADD_SOCKET_SHORTCUT_KEY),
            AddButtonHandler::new(Rc::clone(&controller)),
        );

        let bottom_content = Padded::new(Padding::new(1, 1, 0, 1), VerticalSplit::new(help_text, add_button, 0, 1));

        let top_message = match socket_type {
            SocketPopupType::Socks5 => SOCKS5_TOP_MESSAGE,
            SocketPopupType::Sandstorm => SANDSTORM_TOP_MESSAGE,
        };

        let prompt_text = CenteredText::new(top_message.into(), TOP_MESSAGE_STYLE);
        let content = VerticalSplit::new(upper_content, bottom_content, 0, 0);
        let content = VerticalSplit::new(prompt_text, content, 0, 0);

        let controller_weak = Rc::downgrade(&controller);

        let title = match socket_type {
            SocketPopupType::Socks5 => SOCKS5_TITLE,
            SocketPopupType::Sandstorm => SANDSTORM_TITLE,
        };

        let base = LoadingPopup::new(
            title.into(),
            LOADING_MESSAGE.into(),
            LOADING_STYLE,
            Color::Reset,
            BACKGROUND_COLOR,
            SizeConstraint::new().max(POPUP_WIDTH, MAX_POPUP_HEIGHT),
            controller,
            content,
        );

        let background_task = tokio::task::spawn_local(async move {
            load_sockets_task(controller_weak).await;
        });

        let value = Self { base, background_task };
        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for SocketsPopup<W> {
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
