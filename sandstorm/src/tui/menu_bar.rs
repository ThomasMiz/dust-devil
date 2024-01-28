use std::{cell::RefCell, fmt::Write, ops::Deref, rc::Rc, time::Duration};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Stylize},
    symbols,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget},
};
use tokio::{sync::Notify, task::JoinHandle};

use crate::tui::pretty_print::PrettyByteDisplayer;

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
    ping_frame_text: String,
}

impl MenuBar {
    pub fn new(redraw_notify: Rc<Notify>) -> Self {
        let state = Rc::new(RefCell::new(MenuBarState {
            shutdown_label: format!("{SHUTDOWN_LABEL} ({SHUTDOWN_KEY})"),
            socks5_label: format!("{SOCKS5_LABEL} ({SOCKS5_KEY})"),
            sandstorm_label: format!("{SANDSTORM_LABEL} ({SANDSTORM_KEY})"),
            users_label: format!("{USERS_LABEL} ({USERS_KEY})"),
            auth_label: format!("{AUTH_LABEL} ({AUTH_KEY})"),
            buffer_frame_text: format!("16.1KB ({BUFFER_KEY})"),
            ping_frame_text: String::from("9999ms"),
        }));

        let state1 = Rc::clone(&state);
        let task_handle = tokio::task::spawn_local(async move {
            menu_bar_background_task(state1, redraw_notify).await;
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

async fn menu_bar_background_task(state: Rc<RefCell<MenuBarState>>, redraw_notify: Rc<Notify>) {
    let mut byte_count = 512usize;
    let mut sleep_time = 1000;
    loop {
        tokio::time::sleep(Duration::from_millis(sleep_time)).await;

        let mut state = state.deref().borrow_mut();
        state.buffer_frame_text.clear();
        let _ = write!(state.buffer_frame_text, "{} ({BUFFER_KEY})", PrettyByteDisplayer(byte_count));
        redraw_notify.deref().notify_one();

        byte_count += 512;
        sleep_time = sleep_time.max(50) - 50;
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

        let frame_chunk_border_set = symbols::border::Set {
            bottom_left: symbols::line::DOUBLE.horizontal_up,
            ..symbols::border::DOUBLE
        };

        Paragraph::new(state.shutdown_label.as_str())
            .block(
                Block::default()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .padding(Padding::horizontal(1)),
            )
            .render(menu_layout[0], buf);

        Paragraph::new(state.socks5_label.as_str())
            .block(
                Block::default()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(frame_chunk_border_set)
                    .padding(Padding::horizontal(1)),
            )
            .render(menu_layout[1], buf);

        Paragraph::new(state.sandstorm_label.as_str())
            .block(
                Block::default()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(frame_chunk_border_set)
                    .padding(Padding::horizontal(1)),
            )
            .render(menu_layout[2], buf);

        Paragraph::new(state.users_label.as_str())
            .block(
                Block::default()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(frame_chunk_border_set)
                    .padding(Padding::horizontal(1)),
            )
            .render(menu_layout[3], buf);

        Paragraph::new(state.auth_label.as_str())
            .block(
                Block::default()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(frame_chunk_border_set)
                    .padding(Padding::horizontal(1)),
            )
            .render(menu_layout[4], buf);

        Paragraph::new(state.buffer_frame_text.as_str().fg(Color::Yellow))
            .block(
                Block::default()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(frame_chunk_border_set)
                    .padding(Padding::horizontal(1)),
            )
            .render(menu_layout[5], buf);

        let extra_frame_text = match EXTRA_LABEL.len() {
            l if l + 3 <= menu_layout[6].width as usize => EXTRA_LABEL,
            _ => "",
        };

        Paragraph::new(extra_frame_text)
            .block(
                Block::default()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_set(frame_chunk_border_set)
                    .padding(Padding::horizontal(1)),
            )
            .render(menu_layout[6], buf);

        Paragraph::new(state.ping_frame_text.as_str())
            .block(
                Block::default()
                    .border_type(BorderType::Double)
                    .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
                    .border_set(frame_chunk_border_set)
                    .padding(Padding::horizontal(1)),
            )
            .render(menu_layout[7], buf);
    }
}
