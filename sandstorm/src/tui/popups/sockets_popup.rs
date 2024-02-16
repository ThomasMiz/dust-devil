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
            centered_text::{CenteredText, CenteredTextLine},
            horizontal_split::HorizontalSplit,
            long_list::{LongList, LongListHandler},
            padded::Padded,
            OnEnterResult,
        },
        text_wrapper::{wrap_lines_by_chars, StaticString},
        ui_element::{AutosizeUIElement, HandleEventStatus, PassFocusDirection, UIElement},
        ui_manager::Popup,
    },
};

use super::{
    add_socket_popup::AddSocketPopup,
    close_socket_popup::CloseSocketPopup,
    loading_popup::{LoadingPopup, LoadingPopupController, LoadingPopupControllerInner},
    message_popup::REQUEST_SEND_ERROR_MESSAGE,
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
};

const BACKGROUND_COLOR: Color = Color::Yellow;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightYellow;
const TEXT_COLOR: Color = Color::Black;

const SOCKS5_TITLE: &str = "─Socks5 Sockets";
const SANDSTORM_TITLE: &str = "─Sandstorm Sockets";
const LOADING_MESSAGE: &str = "Getting socket list from the server...";
const LOADING_STYLE: Style = Style::new().fg(TEXT_COLOR);
const SOCKS5_TOP_MESSAGE: &str = "Listening socks5 sockets:";
const SANDSTORM_TOP_MESSAGE: &str = "Listening Sandstorm sockets:";
const TOP_MESSAGE_STYLE: Style = Style::new().fg(TEXT_COLOR);
const HELP_MESSAGE: &str = "Scroll the list with the arrow keys, press (ENTER) on a socket to close it.";
const HELP_MESSAGE_STYLE: Style = Style::new().fg(TEXT_COLOR);
const ADD_SOCKET_BUTTON_TEXT: &str = "[add new socket (a)]";
const ADD_SOCKET_SHORTCUT_KEY: char = 'a';
const POPUP_WIDTH: u16 = 40;
const MAX_POPUP_HEIGHT: u16 = 24;

const IP_FILTER_LABEL: &str = "Filter:";
const FILTER_ALL_STR: &str = "[ALL]";
const FILTER_ALL_SHORTCUT: Option<char> = None;
const FILTER_IPV4_STR: &str = "[IPv4]";
const FILTER_IPV4_SHORTCUT: Option<char> = Some('4');
const FILTER_IPV6_STR: &str = "[IPv6]";
const FILTER_IPV6_SHORTCUT: Option<char> = Some('6');

const MIN_SOCKET_LIST_HEIGHT: usize = 4;

#[derive(Clone, Copy)]
pub enum SocketPopupType {
    Socks5,
    Sandstorm,
}

impl SocketPopupType {
    pub async fn list_sockets<W: AsyncWrite + Unpin + 'static>(
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

    pub async fn add_socket<W: AsyncWrite + Unpin + 'static>(
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

    pub async fn remove_socket<W: AsyncWrite + Unpin + 'static>(
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

struct SocketPopupContent<W: AsyncWrite + Unpin + 'static> {
    top_text: CenteredText,
    ip_filter: Padded<HorizontalSplit<CenteredTextLine, ArrowSelector<FilterArrowHandler<W>>>>,
    socket_list: LongList<SocketListHandler<W>>,
    help_text: Padded<CenteredText>,
    add_button: Padded<CenteredButton<AddButtonHandler<W>>>,
    top_text_height: u16,
    socket_list_height: u16,
    help_text_height: u16,
    focused_element: FocusedElement,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusedElement {
    IpFilter,
    SocketList,
    AddButton,
}

impl FocusedElement {
    fn down(self) -> Self {
        match self {
            Self::IpFilter => Self::SocketList,
            Self::SocketList => Self::AddButton,
            Self::AddButton => Self::AddButton,
        }
    }

    fn up(self) -> Self {
        match self {
            Self::IpFilter => Self::IpFilter,
            Self::SocketList => Self::IpFilter,
            Self::AddButton => Self::SocketList,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::IpFilter => Self::SocketList,
            Self::SocketList => Self::AddButton,
            Self::AddButton => Self::IpFilter,
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
        let ip_filter = match selected_index {
            1 => IpFilterType::IPv4,
            2 => IpFilterType::IPv6,
            _ => IpFilterType::All,
        };

        self.controller.on_ip_filter_changed(ip_filter);
    }
}

impl<W: AsyncWrite + Unpin + 'static> SocketPopupContent<W> {
    fn new(redraw_notify: Rc<Notify>, controller: Rc<Controller<W>>, socket_type: SocketPopupType) -> Self {
        let text_style = Style::new().fg(TEXT_COLOR);
        let selected_text_style = text_style.bg(SELECTED_BACKGROUND_COLOR);

        let ip_filter_selector = ArrowSelector::new(
            Rc::clone(&redraw_notify),
            vec![
                (FILTER_ALL_STR.into(), FILTER_ALL_SHORTCUT),
                (FILTER_IPV4_STR.into(), FILTER_IPV4_SHORTCUT),
                (FILTER_IPV6_STR.into(), FILTER_IPV6_SHORTCUT),
            ],
            0,
            text_style,
            selected_text_style,
            selected_text_style,
            selected_text_style,
            false,
            FilterArrowHandler::new(Rc::clone(&controller)),
        );

        let ip_filter_label = CenteredTextLine::new(IP_FILTER_LABEL.into(), text_style);
        let ip_filter_inner = HorizontalSplit::new(ip_filter_label, ip_filter_selector, 0, 1);
        let ip_filter = Padded::new(Padding::horizontal(1), ip_filter_inner);

        let socket_list = LongList::new(
            Rc::clone(&redraw_notify),
            "".into(),
            0,
            false,
            true,
            SocketListHandler::new(Rc::clone(&controller)),
        );

        let help_text_inner = CenteredText::new(HELP_MESSAGE.into(), HELP_MESSAGE_STYLE);
        let help_text = Padded::new(Padding::horizontal(1), help_text_inner);

        let add_button_inner = CenteredButton::new(
            redraw_notify,
            ADD_SOCKET_BUTTON_TEXT.into(),
            text_style,
            selected_text_style,
            Some(ADD_SOCKET_SHORTCUT_KEY),
            AddButtonHandler::new(Rc::clone(&controller)),
        );
        let add_button = Padded::new(Padding::uniform(1), add_button_inner);

        let top_message = match socket_type {
            SocketPopupType::Socks5 => SOCKS5_TOP_MESSAGE,
            SocketPopupType::Sandstorm => SANDSTORM_TOP_MESSAGE,
        };

        let top_text = CenteredText::new(top_message.into(), TOP_MESSAGE_STYLE);

        Self {
            top_text,
            ip_filter,
            socket_list,
            help_text,
            add_button,
            top_text_height: 1,
            socket_list_height: 1,
            help_text_height: 1,
            focused_element: FocusedElement::IpFilter,
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for SocketPopupContent<W> {
    fn resize(&mut self, area: Rect) {
        if area.is_empty() {
            return;
        }

        let mut remaining_area = area;

        let top_text_size = self.top_text.begin_resize(remaining_area.width, remaining_area.height);
        let mut top_text_area = remaining_area;
        top_text_area.height = top_text_area.height.min(top_text_size.1);
        self.top_text.resize(top_text_area);
        self.top_text_height = top_text_area.height;

        remaining_area.height -= top_text_area.height;
        remaining_area.y = top_text_area.bottom();
        if remaining_area.height == 0 {
            return;
        }

        let mut ip_filter_area = remaining_area;
        ip_filter_area.height = 1;
        self.ip_filter.resize(ip_filter_area);

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

        self.socket_list.resize(remaining_area);
        self.socket_list_height = remaining_area.height;
    }

    fn render(&mut self, mut area: Rect, frame: &mut Frame) {
        if area.height < self.top_text_height {
            return;
        }

        let controller = &self.socket_list.handler.controller;
        if controller.did_list_change() {
            let list = &mut self.socket_list.handler.sockets_filtered;
            list.clear();
            controller.filter_list_into(list);
            let list_len = list.len();
            self.socket_list.reset_items_no_redraw(list_len, true);
        }

        let mut top_text_area = area;
        top_text_area.height = self.top_text_height;
        self.top_text.render(top_text_area, frame);
        area.y += self.top_text_height;
        area.height -= self.top_text_height;
        if area.height == 0 {
            return;
        }

        let ip_filter_area = Rect::new(area.x, area.y, area.width, 1);
        self.ip_filter.render(ip_filter_area, frame);
        area.y += 1;
        area.height -= 1;
        if area.height < self.socket_list_height {
            return;
        }

        let socket_list_area = Rect::new(area.x, area.y, area.width, self.socket_list_height);
        self.socket_list.render(socket_list_area, frame);
        area.y += self.socket_list_height;
        area.height -= self.socket_list_height;
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
                FocusedElement::IpFilter => self.ip_filter.handle_event(event, true),
                FocusedElement::SocketList => self.socket_list.handle_event(event, true),
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
                        FocusedElement::IpFilter => self.ip_filter.receive_focus(focus_position),
                        FocusedElement::SocketList => self.socket_list.receive_focus(focus_position),
                        FocusedElement::AddButton => self.add_button.receive_focus(focus_position),
                    };

                    if !focus_passed {
                        return status;
                    }

                    match self.focused_element {
                        FocusedElement::IpFilter => self.ip_filter.focus_lost(),
                        FocusedElement::SocketList => self.socket_list.focus_lost(),
                        FocusedElement::AddButton => self.add_button.focus_lost(),
                    }

                    self.focused_element = next_focused_element;
                    return HandleEventStatus::Handled;
                }
                HandleEventStatus::Unhandled => {}
            }
        }

        self.ip_filter
            .handle_event(event, false)
            .or_else(|| self.socket_list.handle_event(event, false))
            .or_else(|| self.add_button.handle_event(event, false))
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.focused_element = FocusedElement::IpFilter;
        self.ip_filter.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        match self.focused_element {
            FocusedElement::IpFilter => self.ip_filter.focus_lost(),
            FocusedElement::SocketList => self.socket_list.focus_lost(),
            FocusedElement::AddButton => self.add_button.focus_lost(),
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> AutosizeUIElement for SocketPopupContent<W> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        let (_, top_text_height) = self.top_text.begin_resize(width, height);
        let (_, help_text_height) = self.help_text.begin_resize(width, height);
        let mut request_height = top_text_height.saturating_add(help_text_height);

        let list_len = self.socket_list.handler.controller.inner.borrow().sockets.len();
        let socket_list_height = list_len.max(MIN_SOCKET_LIST_HEIGHT).saturating_add(2).min(u16::MAX as usize) as u16;
        request_height = request_height.saturating_add(socket_list_height);

        self.ip_filter.begin_resize(width, 1);
        let ip_filter_height = 1;
        let add_button_height = 3;
        let additional_height = ip_filter_height + add_button_height;
        request_height = request_height.saturating_add(additional_height);

        (width, request_height.min(height))
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
    fn get_item_lines<F: FnMut(Line<'static>)>(&mut self, index: usize, wrap_width: u16, f: F) {
        let socket_address = self.sockets_filtered[index];
        let text = socket_address.to_string();
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

    fn on_enter(&mut self, index: usize) -> OnEnterResult {
        let inner_guard = self.controller.inner.borrow();
        let inner = inner_guard.deref();

        let popup = CloseSocketPopup::new(
            Rc::clone(&inner.base.base.redraw_notify),
            Weak::clone(&self.controller.manager),
            self.sockets_filtered[index],
            self.controller.socket_type,
            self.controller.sockets_watch.clone(),
            self.controller.popup_sender.clone(),
        );

        let _ = self.controller.popup_sender.send(popup.into());

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
        let inner_guard = self.controller.inner.borrow();
        let inner = inner_guard.deref();

        let popup = AddSocketPopup::new(
            Rc::clone(&inner.base.base.redraw_notify),
            Weak::clone(&self.controller.manager),
            self.controller.socket_type,
            self.controller.sockets_watch.clone(),
            self.controller.popup_sender.clone(),
        );

        let _ = self.controller.popup_sender.send(popup.into());

        OnEnterResult::Handled
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

        let controller_weak = Rc::downgrade(&controller);

        let title = match socket_type {
            SocketPopupType::Socks5 => SOCKS5_TITLE,
            SocketPopupType::Sandstorm => SANDSTORM_TITLE,
        };

        let content = SocketPopupContent::new(redraw_notify, Rc::clone(&controller), socket_type);

        let base = LoadingPopup::new(
            title.into(),
            LOADING_MESSAGE.into(),
            LOADING_STYLE,
            TEXT_COLOR,
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
