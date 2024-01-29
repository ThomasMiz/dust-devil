use std::{cell::RefCell, fmt::Write, ops::Deref, rc::Rc, time::Duration};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Stylize},
    symbols,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget},
};
use tokio::{
    io::AsyncWrite,
    select,
    sync::{broadcast, Notify},
    task::JoinHandle,
};

use crate::{
    sandstorm::MutexedSandstormRequestManager,
    tui::pretty_print::PrettyByteDisplayer,
    utils::futures::{recv_ignore_lagged, run_with_background},
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

/// This string should consist only of spaces. The size of this string defined the amount of dots
/// ('.') to use for the buffer size frame's loading indicator.
const BUFFER_SIZE_LOADING_INDICATOR_SIZE: &str = "     ";
const BUFFER_SIZE_LOADING_INDICATOR_FREQUENCY: Duration = Duration::from_millis(100);

const BUFFER_TEXT_LOADING_COLOR: Color = Color::LightYellow;
const BUFFER_TEXT_DEFAULT_COLOR: Color = Color::Yellow;
const BUFFER_TEXT_HIGHLIGHT_COLOR: Color = Color::LightRed;
const BUFFER_TEXT_HIGHLIGHT_DURATION: Duration = Duration::from_millis(500);

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
    ping_frame_text: String,
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
            ping_frame_text: String::from("9999ms"),
        }));

        let state1 = Rc::clone(&state);
        let task_handle = tokio::task::spawn_local(async move {
            menu_bar_background_task(manager, state1, redraw_notify, buffer_size_watch).await;
        });

        Self { state, task_handle }
    }

    pub fn as_widget(&self) -> MenuBarWidget {
        MenuBarWidget {
            state: Rc::clone(&self.state),
        }
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
            // SAFETY: We edit the string in place. We always ensure to replace ascii chars with ascii char,
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

async fn menu_bar_background_task<W>(
    manager: Rc<MutexedSandstormRequestManager<W>>,
    state: Rc<RefCell<MenuBarState>>,
    redraw_notify: Rc<Notify>,
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
        buffer_size_loading_indicator(&state, &redraw_notify),
        recv_ignore_lagged(&mut buffer_size_receiver),
    )
    .await;

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

pub struct MenuBarWidget {
    state: Rc<RefCell<MenuBarState>>,
}

impl Widget for MenuBarWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
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
                Constraint::Length(state.ping_frame_text.len() as u16 + 4),
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

        let extra_frame_text = match EXTRA_LABEL.len() {
            l if l + 3 <= menu_layout[6].width as usize => EXTRA_LABEL,
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

        Paragraph::new(state.ping_frame_text.as_str())
            .block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
                    .border_set(FRAME_CHUNK_BORDER_SET)
                    .padding(HORIZONTAL_PADDING_ONE),
            )
            .render(menu_layout[7], buf);
    }
}
