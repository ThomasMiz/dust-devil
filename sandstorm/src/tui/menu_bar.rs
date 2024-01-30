use std::{
    cell::RefCell,
    fmt::Write,
    ops::Deref,
    rc::Rc,
    time::{Duration, Instant},
};

use crossterm::event;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Stylize},
    symbols,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget},
};

use tokio::{
    io::AsyncWrite,
    join, select,
    sync::{broadcast, oneshot, Notify},
    task::JoinHandle,
};

use crate::{
    sandstorm::MutexedSandstormRequestManager,
    tui::pretty_print::{PrettyByteDisplayer, PrettyMillisDisplay},
    utils::futures::{recv_ignore_lagged, run_with_background},
};

use super::ui_element::{HandleEventStatus, UIElement};

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

pub struct MenuBar {
    state: Rc<RefCell<MenuBarState>>,
    task_handle: JoinHandle<()>,
}

pub struct MenuBarState {
    shutdown_label: String,
    socks5_label: String,
    sandstorm_label: String,
    users_label: String,
    auth_label: String,
    buffer_frame_text: String,
    buffer_frame_text_color: Color,
    buffer_frame_loading: bool,
    ping_frame_text: String,
    ping_frame_text_color: Color,
    ping_frame_loading: bool,
}

impl MenuBar {
    pub fn new<W>(
        manager: Rc<MutexedSandstormRequestManager<W>>,
        redraw_notify: Rc<Notify>,
        buffer_size_watch: broadcast::Sender<u32>,
    ) -> Self
    where
        W: AsyncWrite + Unpin + 'static,
    {
        let state = Rc::new(RefCell::new(MenuBarState {
            shutdown_label: format!("{SHUTDOWN_LABEL} ({SHUTDOWN_KEY})"),
            socks5_label: format!("{SOCKS5_LABEL} ({SOCKS5_KEY})"),
            sandstorm_label: format!("{SANDSTORM_LABEL} ({SANDSTORM_KEY})"),
            users_label: format!("{USERS_LABEL} ({USERS_KEY})"),
            auth_label: format!("{AUTH_LABEL} ({AUTH_KEY})"),
            buffer_frame_text: format!("{BUFFER_SIZE_LOADING_INDICATOR_SIZE} ({BUFFER_KEY})"),
            buffer_frame_text_color: BUFFER_TEXT_LOADING_COLOR,
            buffer_frame_loading: true,
            ping_frame_text: String::new(),
            ping_frame_text_color: PING_TEXT_LOADING_COLOR,
            ping_frame_loading: true,
        }));

        let state1 = Rc::clone(&state);
        let task_handle = tokio::task::spawn_local(async move {
            menu_bar_background_task(manager, state1, redraw_notify, buffer_size_watch).await;
        });

        Self { state, task_handle }
    }
}

impl Drop for MenuBar {
    fn drop(&mut self) {
        self.task_handle.abort();
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
            let loading_indicator_bytes = state_inner.buffer_frame_text[..(TOTAL_DOTS as usize)].as_bytes_mut();
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
    manager: &Rc<MutexedSandstormRequestManager<W>>,
    state: &Rc<RefCell<MenuBarState>>,
    redraw_notify: &Rc<Notify>,
    buffer_size_sender: broadcast::Sender<u32>,
) where
    W: AsyncWrite + Unpin,
{
    let mut buffer_size_receiver = buffer_size_sender.subscribe();

    let get_buffer_size_result = manager
        .get_buffer_size_fn(move |response| {
            let _ = buffer_size_sender.send(response.0);
        })
        .await;

    if get_buffer_size_result.is_err() {
        return;
    }

    let recv_buffer_size_result = run_with_background(
        buffer_size_loading_indicator(state, redraw_notify),
        recv_ignore_lagged(&mut buffer_size_receiver),
    )
    .await;

    state.deref().borrow_mut().buffer_frame_loading = false;

    match recv_buffer_size_result {
        Ok(buffer_size) => {
            let mut state_inner = state.deref().borrow_mut();
            state_inner.buffer_frame_text.clear();
            let _ = write!(
                state_inner.buffer_frame_text,
                "{} ({BUFFER_KEY})",
                PrettyByteDisplayer(buffer_size as usize)
            );
            state_inner.buffer_frame_text_color = BUFFER_TEXT_HIGHLIGHT_COLOR;
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
                state_inner.buffer_frame_text.clear();
                let _ = write!(
                    state_inner.buffer_frame_text,
                    "{} ({BUFFER_KEY})",
                    PrettyByteDisplayer(buffer_size as usize)
                );
                state_inner.buffer_frame_text_color = BUFFER_TEXT_HIGHLIGHT_COLOR;
                wait_until_highlight_off = true;
            }
            _ = tokio::time::sleep(BUFFER_TEXT_HIGHLIGHT_DURATION), if wait_until_highlight_off => {
                let mut state_inner = state.deref().borrow_mut();
                state_inner.buffer_frame_text_color = BUFFER_TEXT_DEFAULT_COLOR;
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
            let text = &mut state_inner.ping_frame_text;
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

async fn measure_ping<W: AsyncWrite + Unpin>(manager: &Rc<MutexedSandstormRequestManager<W>>) -> Result<u16, ()> {
    let (tx, rx) = oneshot::channel();
    let start_time = Instant::now();
    let meow_result = manager
        .meow_fn(move |_result| {
            let elapsed = start_time.elapsed();
            let elapsed_millis = elapsed.as_millis().min(u16::MAX as u128) as u16;
            let _ = tx.send(elapsed_millis);
        })
        .await;

    if meow_result.is_err() {
        return Err(());
    }

    rx.await.map_err(|_| ())
}

async fn ping_frame_background_task<W>(
    manager: &Rc<MutexedSandstormRequestManager<W>>,
    state: &Rc<RefCell<MenuBarState>>,
    redraw_notify: &Rc<Notify>,
) where
    W: AsyncWrite + Unpin,
{
    let first_ping = run_with_background(ping_frame_loading_indicator(state, redraw_notify), measure_ping(manager)).await;

    state.deref().borrow_mut().ping_frame_loading = false;
    let mut ping_millis = match first_ping {
        Ok(v) => v,
        Err(_) => return,
    };

    let mut previous_ping = !ping_millis;

    loop {
        if previous_ping != ping_millis {
            let mut state_inner = state.deref().borrow_mut();
            let text = &mut state_inner.ping_frame_text;
            text.clear();
            let _ = write!(text, "{}", PrettyMillisDisplay(ping_millis));
            state_inner.ping_frame_text_color = get_ping_text_color(ping_millis);
            previous_ping = ping_millis;
            redraw_notify.notify_one();
        }

        tokio::time::sleep(PING_MEASURE_INTERVAL).await;

        ping_millis = match measure_ping(manager).await {
            Ok(v) => v,
            Err(_) => return,
        };
    }
}

async fn menu_bar_background_task<W>(
    manager: Rc<MutexedSandstormRequestManager<W>>,
    state: Rc<RefCell<MenuBarState>>,
    redraw_notify: Rc<Notify>,
    buffer_size_sender: broadcast::Sender<u32>,
) where
    W: AsyncWrite + Unpin,
{
    join!(
        buffer_frame_background_task(&manager, &state, &redraw_notify, buffer_size_sender),
        ping_frame_background_task(&manager, &state, &redraw_notify),
    );
}

impl UIElement for MenuBar {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let state = self.state.deref().borrow_mut();

        let menu_layout = Layout::new(
            Direction::Horizontal,
            [
                Constraint::Length(state.shutdown_label.len() as u16 + 3),
                Constraint::Length(state.socks5_label.len() as u16 + 3),
                Constraint::Length(state.sandstorm_label.len() as u16 + 3),
                Constraint::Length(state.users_label.len() as u16 + 3),
                Constraint::Length(state.auth_label.len() as u16 + 3),
                Constraint::Length(state.buffer_frame_text.len() as u16 + 3),
                Constraint::Min(0),
                Constraint::Length(state.ping_frame_text.chars().count() as u16 + 4),
            ],
        )
        .split(area);

        const FRAME_CHUNK_BORDER_SET: symbols::border::Set = symbols::border::Set {
            bottom_left: symbols::line::DOUBLE.horizontal_up,
            ..symbols::border::DOUBLE
        };

        const HORIZONTAL_PADDING_ONE: Padding = Padding::horizontal(1);

        Paragraph::new(state.shutdown_label.as_str())
            .block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .padding(HORIZONTAL_PADDING_ONE),
            )
            .render(menu_layout[0], buf);

        Paragraph::new(state.socks5_label.as_str())
            .block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(FRAME_CHUNK_BORDER_SET)
                    .padding(HORIZONTAL_PADDING_ONE),
            )
            .render(menu_layout[1], buf);

        Paragraph::new(state.sandstorm_label.as_str())
            .block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(FRAME_CHUNK_BORDER_SET)
                    .padding(HORIZONTAL_PADDING_ONE),
            )
            .render(menu_layout[2], buf);

        Paragraph::new(state.users_label.as_str())
            .block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(FRAME_CHUNK_BORDER_SET)
                    .padding(HORIZONTAL_PADDING_ONE),
            )
            .render(menu_layout[3], buf);

        Paragraph::new(state.auth_label.as_str())
            .block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(FRAME_CHUNK_BORDER_SET)
                    .padding(HORIZONTAL_PADDING_ONE),
            )
            .render(menu_layout[4], buf);

        Paragraph::new(state.buffer_frame_text.as_str().fg(state.buffer_frame_text_color))
            .block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(FRAME_CHUNK_BORDER_SET)
                    .padding(HORIZONTAL_PADDING_ONE),
            )
            .render(menu_layout[5], buf);

        let extra_frame_text = match state.buffer_frame_loading || state.ping_frame_loading {
            true => EXTRA_LOADING_LABEL,
            false => EXTRA_LABEL,
        };

        let extra_frame_text = match extra_frame_text.len() {
            l if l + 3 <= menu_layout[6].width as usize => extra_frame_text,
            _ => "",
        };

        Paragraph::new(extra_frame_text)
            .block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(FRAME_CHUNK_BORDER_SET)
                    .padding(HORIZONTAL_PADDING_ONE),
            )
            .render(menu_layout[6], buf);

        Paragraph::new(state.ping_frame_text.as_str().fg(state.ping_frame_text_color))
            .block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
                    .border_set(FRAME_CHUNK_BORDER_SET)
                    .padding(HORIZONTAL_PADDING_ONE),
            )
            .render(menu_layout[7], buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        // TODO: Implement event handling
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        // TODO: Implement focus handling
        false
    }

    fn focus_lost(&mut self) {
        // TODO: Implement focus handling
    }
}
