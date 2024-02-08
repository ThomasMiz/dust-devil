use std::{
    cell::RefCell,
    fmt::Write,
    ops::Deref,
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    symbols,
    text::Line,
    widgets::{Block, BorderType, Borders, Padding, Widget},
    Frame,
};

use tokio::{
    io::AsyncWrite,
    join, select,
    sync::{broadcast, mpsc, oneshot, Notify},
    task::JoinHandle,
};

use crate::{
    sandstorm::MutexedSandstormRequestManager,
    tui::pretty_print::{PrettyByteDisplayer, PrettyMillisDisplay},
    utils::futures::{recv_ignore_lagged, run_with_background},
};

use super::{
    popups::shutdown_popup::ShutdownPopup,
    ui_element::{HandleEventStatus, PassFocusDirection, UIElement},
    ui_manager::Popup,
};

const SHUTDOWN_KEY: char = 'x';
const SOCKS5_KEY: char = 's';
const SANDSTORM_KEY: char = 'd';
const USERS_KEY: char = 'u';
const AUTH_KEY: char = 'a';
const BUFFER_KEY: char = 'b';

const SHUTDOWN_LABEL: &str = "Shutdown";
const SOCKS5_LABEL: &str = "Socks5";
const SANDSTORM_LABEL: &str = "Sandstorm";
const USERS_LABEL: &str = "Users";
const AUTH_LABEL: &str = "Auth";
const EXTRA_LABEL: &str = "Sandstorm Protocol v1";
const EXTRA_LOADING_LABEL: &str = "Getting info...";

const SELECTED_BACKGROUND_COLOR: Color = Color::DarkGray;

/// This string should consist only of spaces. The size of this string defined the amount of dots
/// ('.') to use for the buffer size frame's loading indicator.
const BUFFER_SIZE_LOADING_INDICATOR_SIZE: &str = "     ";
const BUFFER_SIZE_LOADING_INDICATOR_FREQUENCY: Duration = Duration::from_millis(100);

const BUFFER_TEXT_LOADING_COLOR: Color = Color::LightYellow;
const BUFFER_TEXT_DEFAULT_COLOR: Color = Color::Yellow;
const BUFFER_TEXT_HIGHLIGHT_COLOR: Color = Color::LightRed;
const BUFFER_TEXT_HIGHLIGHT_DURATION: Duration = Duration::from_millis(500);

const PING_TEXT_LOADING_COLOR: Color = Color::Reset;
const PING_LOADING_INDICATOR_LENGTH: i16 = 4;
const PING_LOADING_INDICATOR_FREQUENCY: Duration = Duration::from_millis(100);
const PING_MEASURE_INTERVAL: Duration = Duration::from_secs(10);

const fn get_ping_text_color(millis: u16) -> Color {
    match millis {
        _ if millis < 100 => Color::Green,
        _ if millis < 1000 => Color::Yellow,
        _ => Color::Red,
    }
}

pub struct MenuBar<W: AsyncWrite + Unpin + 'static> {
    manager: Weak<MutexedSandstormRequestManager<W>>,
    state: Rc<RefCell<MenuBarState>>,
    redraw_notify: Rc<Notify>,
    popup_sender: mpsc::UnboundedSender<Popup>,
    task_handle: JoinHandle<()>,
}

pub struct MenuBarState {
    current_area: Rect,
    shutdown_label: String,
    socks5_label: String,
    socks5_area_x: u16,
    sandstorm_label: String,
    sandstorm_area_x: u16,
    users_label: String,
    users_area_x: u16,
    auth_label: String,
    auth_area_x: u16,
    buffer_label: String,
    buffer_area_x: u16,
    buffer_color: Color,
    buffer_loading: bool,
    buffer_size_was_modified: bool,
    extra_area_x: u16,
    ping_label: String,
    ping_area_x: u16,
    ping_color: Color,
    ping_loading: bool,
    ping_was_modified: bool,
    focused_element: FocusedElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedElement {
    None,
    Shutdown,
    Socks5,
    Sandstorm,
    Users,
    Auth,
    Buffer,
}

impl FocusedElement {
    fn leftmost() -> Self {
        FocusedElement::Shutdown
    }

    fn rightmost() -> Self {
        FocusedElement::Buffer
    }

    fn next_left(self) -> Option<Self> {
        match self {
            Self::Socks5 => Some(Self::Shutdown),
            Self::Sandstorm => Some(Self::Socks5),
            Self::Users => Some(Self::Sandstorm),
            Self::Auth => Some(Self::Users),
            Self::Buffer => Some(Self::Auth),
            _ => None,
        }
    }

    fn next_right(self) -> Option<Self> {
        match self {
            Self::Shutdown => Some(Self::Socks5),
            Self::Socks5 => Some(Self::Sandstorm),
            Self::Sandstorm => Some(Self::Users),
            Self::Users => Some(Self::Auth),
            Self::Auth => Some(Self::Buffer),
            _ => None,
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> MenuBar<W> {
    pub fn new(
        manager: Weak<MutexedSandstormRequestManager<W>>,
        redraw_notify: Rc<Notify>,
        buffer_size_watch: broadcast::Sender<u32>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> Self {
        let state = Rc::new(RefCell::new(MenuBarState {
            current_area: Rect::default(),
            shutdown_label: format!("{SHUTDOWN_LABEL} ({SHUTDOWN_KEY})"),
            socks5_label: format!("{SOCKS5_LABEL} ({SOCKS5_KEY})"),
            socks5_area_x: 0,
            sandstorm_label: format!("{SANDSTORM_LABEL} ({SANDSTORM_KEY})"),
            sandstorm_area_x: 0,
            users_label: format!("{USERS_LABEL} ({USERS_KEY})"),
            users_area_x: 0,
            auth_label: format!("{AUTH_LABEL} ({AUTH_KEY})"),
            auth_area_x: 0,
            buffer_label: format!("{BUFFER_SIZE_LOADING_INDICATOR_SIZE} ({BUFFER_KEY})"),
            buffer_area_x: 0,
            buffer_color: BUFFER_TEXT_LOADING_COLOR,
            buffer_loading: true,
            buffer_size_was_modified: false,
            extra_area_x: 0,
            ping_label: " ".repeat(PING_LOADING_INDICATOR_LENGTH as usize),
            ping_area_x: 0,
            ping_color: PING_TEXT_LOADING_COLOR,
            ping_loading: true,
            ping_was_modified: false,
            focused_element: FocusedElement::None,
        }));

        let task_handle = start_menu_bar_background_task(&manager, &state, &redraw_notify, buffer_size_watch);

        Self {
            manager,
            state,
            task_handle,
            popup_sender,
            redraw_notify,
        }
    }

    fn shutdown_selected(&self) {
        let popup = ShutdownPopup::new(Rc::clone(&self.redraw_notify), Weak::clone(&self.manager));
        let _ = self.popup_sender.send(popup.into());
    }

    fn socsk5_selected(&self) {}

    fn sandstorm_selected(&self) {}

    fn users_selected(&self) {}

    fn auth_selected(&self) {}

    fn buffer_selected(&self) {}
}

impl<W: AsyncWrite + Unpin + 'static> Drop for MenuBar<W> {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

impl MenuBarState {
    fn do_resize(&mut self, new_area: Rect) {
        self.current_area = new_area;
        self.buffer_size_was_modified = false;
        self.ping_was_modified = false;

        self.socks5_area_x = (self.shutdown_label.len() as u16 + 3).min(new_area.width);
        self.sandstorm_area_x = (self.socks5_area_x + self.socks5_label.len() as u16 + 3).min(new_area.width);
        self.users_area_x = (self.sandstorm_area_x + self.sandstorm_label.len() as u16 + 3).min(new_area.width);
        self.auth_area_x = (self.users_area_x + self.users_label.len() as u16 + 3).min(new_area.width);
        self.buffer_area_x = (self.auth_area_x + self.auth_label.len() as u16 + 3).min(new_area.width);
        self.extra_area_x = (self.buffer_area_x + self.buffer_label.len() as u16 + 3).min(new_area.width);

        let ping_area_width = (new_area.width - self.extra_area_x).min(self.ping_label.chars().count() as u16 + 4);
        self.ping_area_x = new_area.width - ping_area_width;
    }

    fn get_focus_position(&self) -> (u16, u16) {
        let x = match self.focused_element {
            FocusedElement::Shutdown => self.socks5_area_x / 2,
            FocusedElement::Socks5 => self.socks5_area_x + (self.sandstorm_area_x - self.socks5_area_x) / 2,
            FocusedElement::Sandstorm => self.sandstorm_area_x + (self.users_area_x - self.sandstorm_area_x) / 2,
            FocusedElement::Users => self.users_area_x + (self.auth_area_x - self.users_area_x) / 2,
            FocusedElement::Auth => self.auth_area_x + (self.buffer_area_x - self.auth_area_x) / 2,
            FocusedElement::Buffer => self.buffer_area_x + (self.extra_area_x - self.buffer_area_x) / 2,
            FocusedElement::None => self.current_area.width / 2,
        };

        (self.current_area.x + x, self.current_area.y)
    }

    fn shutdown_area(&self) -> Rect {
        let width = self.socks5_area_x;
        Rect::new(self.current_area.x, self.current_area.y, width, self.current_area.height)
    }

    fn socks5_area(&self) -> Rect {
        let x = self.current_area.x + self.socks5_area_x;
        let width = self.sandstorm_area_x.saturating_sub(self.socks5_area_x);
        Rect::new(x, self.current_area.y, width, self.current_area.height)
    }

    fn sandstorm_area(&self) -> Rect {
        let x = self.current_area.x + self.sandstorm_area_x;
        let width = self.users_area_x.saturating_sub(self.sandstorm_area_x);
        Rect::new(x, self.current_area.y, width, self.current_area.height)
    }

    fn users_area(&self) -> Rect {
        let x = self.current_area.x + self.users_area_x;
        let width = self.auth_area_x.saturating_sub(self.users_area_x);
        Rect::new(x, self.current_area.y, width, self.current_area.height)
    }

    fn auth_area(&self) -> Rect {
        let x = self.current_area.x + self.auth_area_x;
        let width = self.buffer_area_x.saturating_sub(self.auth_area_x);
        Rect::new(x, self.current_area.y, width, self.current_area.height)
    }

    fn buffer_area(&self) -> Rect {
        let x = self.current_area.x + self.buffer_area_x;
        let width = self.extra_area_x.saturating_sub(self.buffer_area_x);
        Rect::new(x, self.current_area.y, width, self.current_area.height)
    }

    fn extra_area(&self) -> Rect {
        let x = self.current_area.x + self.extra_area_x;
        let width = self.ping_area_x.saturating_sub(self.extra_area_x);
        Rect::new(x, self.current_area.y, width, self.current_area.height)
    }

    fn ping_area(&self) -> Rect {
        let x = self.current_area.x + self.ping_area_x;
        let width = self.current_area.right().saturating_sub(self.ping_area_x);
        Rect::new(x, self.current_area.y, width, self.current_area.height)
    }
}

async fn buffer_size_loading_indicator(state: &Rc<RefCell<MenuBarState>>, redraw_notify: &Rc<Notify>) {
    const TOTAL_DOTS: i32 = BUFFER_SIZE_LOADING_INDICATOR_SIZE.len() as i32;

    let mut current_dots = 0;
    let mut interval = tokio::time::interval(BUFFER_SIZE_LOADING_INDICATOR_FREQUENCY);
    loop {
        interval.tick().await;
        let mut state_inner = state.deref().borrow_mut();

        unsafe {
            // SAFETY: We edit the string in place. We always ensure to replace ascii chars with ascii chars,
            // so the string remains valid UTF-8.
            let loading_indicator_bytes = state_inner.buffer_label[..(TOTAL_DOTS as usize)].as_bytes_mut();
            if current_dots == 0 {
                if !loading_indicator_bytes.is_ascii() {
                    panic!("Someone's fucking with the loading indicator AND IT AIN'T ME!");
                }
                loading_indicator_bytes.fill(b' ');
            } else {
                let i = (current_dots - 1) as usize;
                if !loading_indicator_bytes[i].is_ascii() {
                    panic!("Someone's fucking with the loading indicator AND IT AIN'T ME!");
                }
                loading_indicator_bytes[i] = b'.';
            }
        }

        current_dots = (current_dots + 1) % (TOTAL_DOTS + 1);
        redraw_notify.notify_one();
    }
}

async fn buffer_frame_background_task<W>(
    manager: &Weak<MutexedSandstormRequestManager<W>>,
    state: &Rc<RefCell<MenuBarState>>,
    redraw_notify: &Rc<Notify>,
    buffer_size_sender: broadcast::Sender<u32>,
) where
    W: AsyncWrite + Unpin,
{
    let mut buffer_size_receiver = buffer_size_sender.subscribe();

    let manager_rc = match manager.upgrade() {
        Some(rc) => rc,
        None => return,
    };

    let get_buffer_size_result = manager_rc
        .get_buffer_size_fn(move |response| {
            let _ = buffer_size_sender.send(response.0);
        })
        .await;
    drop(manager_rc);

    if get_buffer_size_result.is_err() {
        return;
    }

    let recv_buffer_size_result = run_with_background(
        buffer_size_loading_indicator(state, redraw_notify),
        recv_ignore_lagged(&mut buffer_size_receiver),
    )
    .await;

    state.deref().borrow_mut().buffer_loading = false;

    match recv_buffer_size_result {
        Ok(buffer_size) => {
            let mut state_inner = state.deref().borrow_mut();
            state_inner.buffer_label.clear();
            let _ = write!(
                state_inner.buffer_label,
                "{} ({BUFFER_KEY})",
                PrettyByteDisplayer(buffer_size as usize)
            );
            state_inner.buffer_color = BUFFER_TEXT_HIGHLIGHT_COLOR;
            state_inner.buffer_size_was_modified = true;
        }
        Err(_) => return,
    };

    let mut wait_until_highlight_off = true;

    loop {
        redraw_notify.notify_one();

        select! {
            biased;
            buffer_size_recv = recv_ignore_lagged(&mut buffer_size_receiver) => {
                let buffer_size = match buffer_size_recv {
                    Ok(v) => v,
                    Err(_) => return,
                };

                let mut state_inner = state.deref().borrow_mut();
                state_inner.buffer_label.clear();
                let _ = write!(
                    state_inner.buffer_label,
                    "{} ({BUFFER_KEY})",
                    PrettyByteDisplayer(buffer_size as usize)
                );
                state_inner.buffer_color = BUFFER_TEXT_HIGHLIGHT_COLOR;
                wait_until_highlight_off = true;
                state_inner.buffer_size_was_modified = true;
            }
            _ = tokio::time::sleep(BUFFER_TEXT_HIGHLIGHT_DURATION), if wait_until_highlight_off => {
                let mut state_inner = state.deref().borrow_mut();
                state_inner.buffer_color = BUFFER_TEXT_DEFAULT_COLOR;
                wait_until_highlight_off = false;
            }
        }
    }
}

async fn ping_frame_loading_indicator(state: &Rc<RefCell<MenuBarState>>, redraw_notify: &Rc<Notify>) {
    let mut current_i = PING_LOADING_INDICATOR_LENGTH / 2;
    let mut direction = 1;
    loop {
        {
            let mut state_inner = state.deref().borrow_mut();
            let text = &mut state_inner.ping_label;
            text.clear();
            for _ in 0..current_i {
                text.push(symbols::half_block::LOWER);
            }
            text.push(symbols::half_block::FULL);
            for _ in (current_i + 1)..PING_LOADING_INDICATOR_LENGTH {
                text.push(symbols::half_block::LOWER);
            }
        }
        redraw_notify.notify_one();

        tokio::time::sleep(PING_LOADING_INDICATOR_FREQUENCY).await;

        current_i += direction;
        if current_i == -1 {
            current_i = 1;
            direction = 1;
        } else if current_i == PING_LOADING_INDICATOR_LENGTH {
            current_i = PING_LOADING_INDICATOR_LENGTH - 2;
            direction = -1;
        }
    }
}

async fn measure_ping<W: AsyncWrite + Unpin>(manager: &Weak<MutexedSandstormRequestManager<W>>) -> Result<u16, ()> {
    let manager_rc = manager.upgrade().ok_or(())?;

    let (tx, rx) = oneshot::channel();
    let start_time = Instant::now();
    let meow_result = manager_rc
        .meow_fn(move |_result| {
            let elapsed = start_time.elapsed();
            let elapsed_millis = elapsed.as_millis().min(u16::MAX as u128) as u16;
            let _ = tx.send(elapsed_millis);
        })
        .await;

    drop(manager_rc);

    if meow_result.is_err() {
        return Err(());
    }

    rx.await.map_err(|_| ())
}

async fn ping_frame_background_task<W>(
    manager: &Weak<MutexedSandstormRequestManager<W>>,
    state: &Rc<RefCell<MenuBarState>>,
    redraw_notify: &Rc<Notify>,
) where
    W: AsyncWrite + Unpin,
{
    let first_ping = run_with_background(ping_frame_loading_indicator(state, redraw_notify), measure_ping(manager)).await;

    state.deref().borrow_mut().ping_loading = false;
    let mut ping_millis = match first_ping {
        Ok(v) => v,
        Err(_) => return,
    };

    let mut previous_ping = !ping_millis;

    loop {
        if previous_ping != ping_millis {
            let mut state_inner = state.deref().borrow_mut();
            let text = &mut state_inner.ping_label;
            text.clear();
            let _ = write!(text, "{}", PrettyMillisDisplay(ping_millis));
            state_inner.ping_color = get_ping_text_color(ping_millis);
            previous_ping = ping_millis;
            state_inner.ping_was_modified = true;
            redraw_notify.notify_one();
        }

        tokio::time::sleep(PING_MEASURE_INTERVAL).await;

        ping_millis = match measure_ping(manager).await {
            Ok(v) => v,
            Err(_) => return,
        };
    }
}

fn start_menu_bar_background_task<W>(
    manager: &Weak<MutexedSandstormRequestManager<W>>,
    state: &Rc<RefCell<MenuBarState>>,
    redraw_notify: &Rc<Notify>,
    buffer_size_sender: broadcast::Sender<u32>,
) -> JoinHandle<()>
where
    W: AsyncWrite + Unpin + 'static,
{
    let manager = Weak::clone(manager);
    let state = Rc::clone(state);
    let redraw_notify = Rc::clone(redraw_notify);

    tokio::task::spawn_local(async move {
        join!(
            buffer_frame_background_task(&manager, &state, &redraw_notify, buffer_size_sender),
            ping_frame_background_task(&manager, &state, &redraw_notify),
        );
    })
}

#[inline]
fn render_frame_chunk(block: Block, label: &str, area: Rect, style: Style, buf: &mut Buffer) {
    let inner = block.inner(area);
    block.render(area, buf);
    if !inner.is_empty() {
        buf.set_line(inner.x, inner.y, &Line::styled(label, style), inner.width);
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for MenuBar<W> {
    fn resize(&mut self, area: Rect) {
        self.state.deref().borrow_mut().do_resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let mut state = self.state.deref().borrow_mut();
        if state.buffer_size_was_modified || state.ping_was_modified {
            state.do_resize(area);
        }

        const FRAME_CHUNK_BORDER_SET: symbols::border::Set = symbols::border::Set {
            bottom_left: symbols::line::DOUBLE.horizontal_up,
            ..symbols::border::DOUBLE
        };

        const HORIZONTAL_PADDING_ONE: Padding = Padding::horizontal(1);

        const LEFTMOST_FRAME: Block = Block::new()
            .border_type(BorderType::Double)
            .borders(Borders::LEFT.union(Borders::BOTTOM))
            .padding(HORIZONTAL_PADDING_ONE);

        const MIDDLE_FRAME: Block = Block::new()
            .border_set(FRAME_CHUNK_BORDER_SET)
            .borders(Borders::LEFT.union(Borders::BOTTOM))
            .padding(HORIZONTAL_PADDING_ONE);

        const RIGHTMOST: Block = Block::new()
            .border_set(FRAME_CHUNK_BORDER_SET)
            .borders(Borders::LEFT.union(Borders::BOTTOM).union(Borders::RIGHT))
            .padding(HORIZONTAL_PADDING_ONE);

        const STYLE: Style = Style::reset();

        let mut shutdown_style = STYLE;
        let mut socks5_style = STYLE;
        let mut sandstorm_style = STYLE;
        let mut users_style = STYLE;
        let mut auth_style = STYLE;
        let mut buffer_style = STYLE.fg(state.buffer_color);

        let style_of_focused = match state.focused_element {
            FocusedElement::None => None,
            FocusedElement::Shutdown => Some(&mut shutdown_style),
            FocusedElement::Socks5 => Some(&mut socks5_style),
            FocusedElement::Sandstorm => Some(&mut sandstorm_style),
            FocusedElement::Users => Some(&mut users_style),
            FocusedElement::Auth => Some(&mut auth_style),
            FocusedElement::Buffer => Some(&mut buffer_style),
        };

        if let Some(style) = style_of_focused {
            *style = style.bg(SELECTED_BACKGROUND_COLOR);
        }

        let buf = frame.buffer_mut();
        render_frame_chunk(LEFTMOST_FRAME, &state.shutdown_label, state.shutdown_area(), shutdown_style, buf);
        render_frame_chunk(MIDDLE_FRAME, &state.socks5_label, state.socks5_area(), socks5_style, buf);
        render_frame_chunk(MIDDLE_FRAME, &state.sandstorm_label, state.sandstorm_area(), sandstorm_style, buf);
        render_frame_chunk(MIDDLE_FRAME, &state.users_label, state.users_area(), users_style, buf);
        render_frame_chunk(MIDDLE_FRAME, &state.auth_label, state.auth_area(), auth_style, buf);
        render_frame_chunk(MIDDLE_FRAME, &state.buffer_label, state.buffer_area(), buffer_style, buf);

        let extra_frame_text = match state.buffer_loading || state.ping_loading {
            true => EXTRA_LOADING_LABEL,
            false => EXTRA_LABEL,
        };

        let extra_frame_text = match extra_frame_text.len() {
            l if l + 3 <= state.extra_area().width as usize => extra_frame_text,
            _ => "",
        };

        render_frame_chunk(MIDDLE_FRAME, extra_frame_text, state.extra_area(), STYLE, buf);

        let ping_col = Style::reset().fg(state.ping_color);
        render_frame_chunk(RIGHTMOST, &state.ping_label, state.ping_area(), ping_col, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        let key_event = match event {
            event::Event::Key(e) if e.kind != KeyEventKind::Release => e,
            _ => return HandleEventStatus::Unhandled,
        };

        let mut state_inner = self.state.deref().borrow_mut();

        if let KeyCode::Char(c) = key_event.code {
            let mut handled = true;

            match c {
                SHUTDOWN_KEY => self.shutdown_selected(),
                SOCKS5_KEY => self.socsk5_selected(),
                SANDSTORM_KEY => self.sandstorm_selected(),
                USERS_KEY => self.users_selected(),
                AUTH_KEY => self.auth_selected(),
                BUFFER_KEY => self.buffer_selected(),
                _ => handled = false,
            }

            return match handled {
                true => HandleEventStatus::Handled,
                false => HandleEventStatus::Unhandled,
            };
        }

        if !is_focused {
            return HandleEventStatus::Unhandled;
        }

        let previous_focused_element = state_inner.focused_element;

        let result = match key_event.code {
            KeyCode::Enter => {
                let mut handled = true;

                match state_inner.focused_element {
                    FocusedElement::Shutdown => self.shutdown_selected(),
                    FocusedElement::Socks5 => self.socsk5_selected(),
                    FocusedElement::Sandstorm => self.sandstorm_selected(),
                    FocusedElement::Users => self.users_selected(),
                    FocusedElement::Auth => self.auth_selected(),
                    FocusedElement::Buffer => self.buffer_selected(),
                    _ => handled = false,
                }

                match handled {
                    true => HandleEventStatus::Handled,
                    false => HandleEventStatus::Unhandled,
                }
            }
            KeyCode::Left => {
                if state_inner.focused_element == FocusedElement::None {
                    state_inner.focused_element = FocusedElement::rightmost();
                    HandleEventStatus::Handled
                } else if let Some(e) = state_inner.focused_element.next_left() {
                    state_inner.focused_element = e;
                    HandleEventStatus::Handled
                } else {
                    HandleEventStatus::PassFocus(state_inner.get_focus_position(), PassFocusDirection::Left)
                }
            }
            KeyCode::Right | KeyCode::Tab => {
                if state_inner.focused_element == FocusedElement::None {
                    state_inner.focused_element = FocusedElement::leftmost();
                    HandleEventStatus::Handled
                } else if let Some(e) = state_inner.focused_element.next_right() {
                    state_inner.focused_element = e;
                    HandleEventStatus::Handled
                } else {
                    let direction = match key_event.code {
                        KeyCode::Right => PassFocusDirection::Right,
                        _ => PassFocusDirection::Forward,
                    };

                    HandleEventStatus::PassFocus(state_inner.get_focus_position(), direction)
                }
            }
            KeyCode::Down => HandleEventStatus::PassFocus(state_inner.get_focus_position(), PassFocusDirection::Down),
            KeyCode::Up => HandleEventStatus::PassFocus(state_inner.get_focus_position(), PassFocusDirection::Up),
            KeyCode::Esc => HandleEventStatus::PassFocus(state_inner.get_focus_position(), PassFocusDirection::Away),
            _ => HandleEventStatus::Unhandled,
        };

        if state_inner.focused_element != previous_focused_element {
            self.redraw_notify.notify_one();
        }

        result
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        let mut state_inner = self.state.deref().borrow_mut();

        state_inner.focused_element = match focus_position {
            (x, _) if x < state_inner.socks5_area_x => FocusedElement::Shutdown,
            (x, _) if x < state_inner.sandstorm_area_x => FocusedElement::Socks5,
            (x, _) if x < state_inner.users_area_x => FocusedElement::Sandstorm,
            (x, _) if x < state_inner.auth_area_x => FocusedElement::Users,
            (x, _) if x < state_inner.buffer_area_x => FocusedElement::Auth,
            _ => FocusedElement::Buffer,
        };

        self.redraw_notify.notify_one();
        true
    }

    fn focus_lost(&mut self) {
        self.state.deref().borrow_mut().focused_element = FocusedElement::None;
        self.redraw_notify.notify_one();
    }
}
