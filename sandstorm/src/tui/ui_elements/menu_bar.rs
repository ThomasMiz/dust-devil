/*
██████████████████████████████████████████████████████████████████████████████████████████████████████████████████████████
█║ Shutdown (x) ║ Socks5 (s) ║ Sandstorm (d) ║ Users (u) ║ Auth (a) ║ 16.9KB (b) ║ Sandstorm Protocol v1       ║ 9999ms ║█
█╚══════════════╩════════════╩═══════════════╩═══════════╩══════════╩════════════╩═════════════════════════════╩════════╝█
*/

use std::{
    borrow::BorrowMut,
    cell::RefCell,
    fmt::Write,
    io::{self, Error},
    num::NonZeroU16,
    ops::Deref,
    rc::{Rc, Weak},
    time::Duration,
};

use crossterm::{
    style::{ContentStyle, Print, SetStyle, Stylize},
    QueueableCommand,
};
use tokio::{sync::Notify, task::JoinHandle};

use crate::tui::{
    chars,
    pretty_print::PrettyByteDisplayer,
    types::{HorizontalLine, Point, Rectangle},
};

use super::{simple_label::SimpleLabel, utils::ensure_cursor_at_start, UIElement};

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

const SHUTDOWN_FRAME_OFFSET: u16 = 0;
const SOCKS5_FRAME_OFFSET: u16 = SHUTDOWN_FRAME_OFFSET + SHUTDOWN_LABEL.len() as u16 + 7;
const SANDSTORM_FRAME_OFFSET: u16 = SOCKS5_FRAME_OFFSET + SOCKS5_LABEL.len() as u16 + 7;
const USERS_FRAME_OFFSET: u16 = SANDSTORM_FRAME_OFFSET + SANDSTORM_LABEL.len() as u16 + 7;
const AUTH_FRAME_OFFSET: u16 = USERS_FRAME_OFFSET + USERS_LABEL.len() as u16 + 7;
const BUFFER_FRAME_OFFSET: u16 = AUTH_FRAME_OFFSET + AUTH_LABEL.len() as u16 + 7;

/// A frame chunk is part of a frame that looks like this:
/// ```txt
/// ║ {label} ({key})
/// ╚═════════════════
/// ```
///
/// This allows concatenating many frame chunks to form a menu bar.
struct FrameChunk {
    area: Rectangle,
    label: SimpleLabel,
    is_leftmost: bool,
    is_rightmost: bool,
    frame_style: ContentStyle,
    needs_redraw: bool,
}

impl FrameChunk {
    pub fn new(
        area: Rectangle,
        label_text: &str,
        maybe_key: Option<char>,
        is_leftmost: bool,
        is_rightmost: bool,
        text_style: ContentStyle,
        frame_style: ContentStyle,
    ) -> Self {
        let mut text = String::with_capacity(label_text.len() + maybe_key.map(|c| c.len_utf8() + 3).unwrap_or(0));
        text.push_str(label_text);
        if let Some(key) = maybe_key {
            text.push_str(" (");
            text.push(key);
            text.push(')');
        }

        let mut label = SimpleLabel::new(HorizontalLine::get_single_pixel_line(), text, text_style);
        let label_start_x = area.left() + 2;
        let label_end_x = area.right().min(label_start_x + label.text_len() - 1);
        if let Some(label_area) = HorizontalLine::from_borders(area.top(), label_start_x, label_end_x) {
            label.resize(label_area.into());
        }

        Self {
            area,
            label,
            is_leftmost,
            is_rightmost,
            frame_style,
            needs_redraw: false,
        }
    }

    pub fn new2(
        area: Rectangle,
        label_text: String,
        is_leftmost: bool,
        is_rightmost: bool,
        text_style: ContentStyle,
        frame_style: ContentStyle,
    ) -> Self {
        let mut label = SimpleLabel::new(HorizontalLine::get_single_pixel_line(), label_text, text_style);
        let label_start_x = area.left() + 2;
        let label_end_x = area.right().min(label_start_x + label.text_len() - 1);
        if let Some(label_area) = HorizontalLine::from_borders(area.top(), label_start_x, label_end_x) {
            label.resize(label_area.into());
        }

        Self {
            area,
            label,
            is_leftmost,
            is_rightmost,
            frame_style,
            needs_redraw: false,
        }
    }

    fn draw_top<O: io::Write>(
        &mut self,
        out: &mut O,
        area: HorizontalLine,
        is_cursor_at_start: &mut bool,
        force_redraw: bool,
    ) -> Result<(), Error> {
        let force_redraw = force_redraw | self.needs_redraw;
        self.needs_redraw = false;

        if force_redraw {
            ensure_cursor_at_start(is_cursor_at_start, out, area.left(), area.y)?;

            let include_vertical = area.left() == self.area.left();
            let space_x = self.area.left() + 1;
            let include_space = area.left() <= space_x && space_x <= area.right();

            if include_vertical || include_space {
                out.queue(SetStyle(self.frame_style))?;
                if include_vertical {
                    out.queue(Print(chars::DOUBLE_VERTICAL))?;
                }

                if include_space {
                    out.queue(Print(" "))?;
                }
            }
        } else if area.left() != self.area.left() + 2 {
            *is_cursor_at_start = false;
        }

        if let Some(label_draw_area) = area.intersection_with_line(self.label.area_as_line()) {
            *is_cursor_at_start = self.label.draw_line(out, label_draw_area, *is_cursor_at_start, force_redraw)?;
        }

        if force_redraw {
            let spaces_area = HorizontalLine::from_borders(area.y, self.area().left() + self.label.text_len() + 2, self.area.right());
            if let Some(Some(spaces_draw_area)) = spaces_area.map(|line| line.intersection_with_line(area)) {
                ensure_cursor_at_start(is_cursor_at_start, out, spaces_draw_area.left(), area.y)?;
                out.queue(SetStyle(self.frame_style))?;

                let end = match self.is_rightmost {
                    true => spaces_draw_area.right().min(self.area.right() - 1),
                    false => spaces_draw_area.right(),
                };

                for _ in spaces_draw_area.left()..=end {
                    out.queue(Print(" "))?;
                }

                if self.is_rightmost && spaces_draw_area.right() == self.area.right() {
                    out.queue(Print(chars::DOUBLE_VERTICAL))?;
                }
            }
        } else if area.width() > self.label.text_len() + 2 {
            *is_cursor_at_start = false;
        }

        Ok(())
    }

    fn draw_bottom<O: io::Write>(
        &self,
        out: &mut O,
        area: HorizontalLine,
        is_cursor_at_start: &mut bool,
        force_redraw: bool,
    ) -> Result<(), Error> {
        if force_redraw {
            ensure_cursor_at_start(is_cursor_at_start, out, area.left(), area.y)?;
            out.queue(SetStyle(self.frame_style))?;

            if area.left() == self.area.left() {
                out.queue(Print(if self.is_leftmost {
                    chars::DOUBLE_BOTTOM_LEFT_CORNER
                } else {
                    chars::DOUBLE_HORIZONTAL_UP
                }))?;
            }

            let end = match self.is_rightmost {
                true => area.right().min(self.area.right() - 1),
                false => area.right(),
            };

            for _ in (area.left().max(self.area.left() + 1))..=end {
                out.queue(Print(chars::DOUBLE_HORIZONTAL))?;
            }

            if self.is_rightmost && area.right() == self.area.right() {
                out.queue(Print(chars::DOUBLE_BOTTOM_RIGHT_CORNER))?;
            }
        }

        Ok(())
    }
}

impl UIElement for FrameChunk {
    fn area(&self) -> Rectangle {
        self.area
    }

    fn draw_line<O: io::Write>(
        &mut self,
        out: &mut O,
        area: HorizontalLine,
        mut is_cursor_at_start: bool,
        force_redraw: bool,
    ) -> Result<bool, Error> {
        if area.y == self.area.top() {
            self.draw_top(out, area, &mut is_cursor_at_start, force_redraw)?;
        } else {
            self.draw_bottom(out, area, &mut is_cursor_at_start, force_redraw)?;
        }

        Ok(is_cursor_at_start)
    }

    fn resize(&mut self, area: Rectangle) {
        self.area = area;
    }
}

struct MenuBarInner {
    shutdown_frame_chunk: FrameChunk,
    socks5_frame_chunk: FrameChunk,
    sandstorm_frame_chunk: FrameChunk,
    users_frame_chunk: FrameChunk,
    auth_frame_chunk: FrameChunk,
    buffer_frame_chunk: FrameChunk,
}

pub struct MenuBar {
    area: Rectangle,
    inner: Rc<RefCell<MenuBarInner>>,
    task_handle: JoinHandle<()>,
}

async fn menu_bar_task(inner_weak: Weak<RefCell<MenuBarInner>>, redraw_notify_weak: Weak<Notify>) {
    let mut byte_count = 1024usize;
    let mut sleep_time = 1000;
    loop {
        tokio::time::sleep(Duration::from_millis(sleep_time)).await;
        sleep_time = sleep_time.max(50) - 50;
        let inner_rc = match inner_weak.upgrade() {
            Some(v) => v,
            None => return,
        };
        let mut inner = inner_rc.deref().borrow_mut();

        let redraw_notify = match redraw_notify_weak.upgrade() {
            Some(v) => v,
            None => return,
        };

        inner.buffer_frame_chunk.label.modify_text(|text| {
            text.clear();
            let _ = write!(text, "{} ({BUFFER_KEY})", PrettyByteDisplayer(byte_count));
        });

        let mut buffer_frame_area = inner.buffer_frame_chunk.area();
        buffer_frame_area.width = NonZeroU16::new(inner.buffer_frame_chunk.label.text_len() + 3).unwrap();
        if buffer_frame_area.width() != inner.buffer_frame_chunk.area().width() {
            inner.buffer_frame_chunk.resize(buffer_frame_area);
        }

        inner.buffer_frame_chunk.needs_redraw = true;
        byte_count += 512;
        redraw_notify.notify_one();
    }
}

impl MenuBar {
    pub fn new(area: Rectangle, redraw_notify: Weak<Notify>) -> Self {
        let frame_style = ContentStyle::default().reset();
        let shutdown_frame_chunk = FrameChunk::new(
            Rectangle::new(
                Point::new(area.left() + SHUTDOWN_FRAME_OFFSET, area.top()),
                NonZeroU16::new(SOCKS5_FRAME_OFFSET - SHUTDOWN_FRAME_OFFSET).unwrap(),
                area.height,
            ),
            SHUTDOWN_LABEL,
            Some(SHUTDOWN_KEY),
            true,
            false,
            ContentStyle::default().red(),
            frame_style,
        );

        let socks5_frame_chunk = FrameChunk::new(
            Rectangle::new(
                Point::new(area.left() + SOCKS5_FRAME_OFFSET, area.top()),
                NonZeroU16::new(SANDSTORM_FRAME_OFFSET - SOCKS5_FRAME_OFFSET).unwrap(),
                area.height,
            ),
            SOCKS5_LABEL,
            Some(SOCKS5_KEY),
            false,
            false,
            ContentStyle::default().red(),
            frame_style,
        );

        let sandstorm_frame_chunk = FrameChunk::new(
            Rectangle::new(
                Point::new(area.left() + SANDSTORM_FRAME_OFFSET, area.top()),
                NonZeroU16::new(USERS_FRAME_OFFSET - SANDSTORM_FRAME_OFFSET).unwrap(),
                area.height,
            ),
            SANDSTORM_LABEL,
            Some(SANDSTORM_KEY),
            false,
            false,
            ContentStyle::default().red(),
            frame_style,
        );

        let users_frame_chunk = FrameChunk::new(
            Rectangle::new(
                Point::new(area.left() + USERS_FRAME_OFFSET, area.top()),
                NonZeroU16::new(AUTH_FRAME_OFFSET - USERS_FRAME_OFFSET).unwrap(),
                area.height,
            ),
            USERS_LABEL,
            Some(USERS_KEY),
            false,
            false,
            ContentStyle::default().red(),
            frame_style,
        );

        let auth_frame_chunk = FrameChunk::new(
            Rectangle::new(
                Point::new(area.left() + AUTH_FRAME_OFFSET, area.top()),
                NonZeroU16::new(BUFFER_FRAME_OFFSET - AUTH_FRAME_OFFSET).unwrap(),
                area.height,
            ),
            AUTH_LABEL,
            Some(AUTH_KEY),
            false,
            false,
            ContentStyle::default().red(),
            frame_style,
        );

        let buffer_frame_text = format!("16.3KB ({BUFFER_KEY})");
        let buffer_frame_chunk = FrameChunk::new2(
            Rectangle::new(
                Point::new(area.left() + BUFFER_FRAME_OFFSET, area.top()),
                NonZeroU16::new(buffer_frame_text.len() as u16 + 3).unwrap(),
                area.height,
            ),
            buffer_frame_text,
            false,
            false,
            ContentStyle::default().red(),
            frame_style,
        );

        let inner = Rc::new(RefCell::new(MenuBarInner {
            shutdown_frame_chunk,
            socks5_frame_chunk,
            sandstorm_frame_chunk,
            users_frame_chunk,
            auth_frame_chunk,
            buffer_frame_chunk,
        }));

        let inner_weak = Rc::downgrade(&inner);
        let task_handle = tokio::task::spawn_local(async move {
            menu_bar_task(inner_weak, redraw_notify).await;
        });

        Self { area, inner, task_handle }
    }
}

impl Drop for MenuBar {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

impl UIElement for MenuBar {
    fn area(&self) -> Rectangle {
        self.area
    }

    fn draw_line<O: io::Write>(
        &mut self,
        out: &mut O,
        area: HorizontalLine,
        mut is_cursor_at_start: bool,
        force_redraw: bool,
    ) -> Result<bool, Error> {
        let mut inner = self.inner.deref().borrow_mut();

        if let Some(shutdown_draw_area) = area.intersection_with_rect(inner.shutdown_frame_chunk.area()) {
            is_cursor_at_start = inner
                .shutdown_frame_chunk
                .draw_line(out, shutdown_draw_area, is_cursor_at_start, force_redraw)?;
        }

        if let Some(socks5_draw_area) = area.intersection_with_rect(inner.socks5_frame_chunk.area()) {
            is_cursor_at_start = inner
                .socks5_frame_chunk
                .draw_line(out, socks5_draw_area, is_cursor_at_start, force_redraw)?;
        }

        if let Some(sandstorm_draw_area) = area.intersection_with_rect(inner.sandstorm_frame_chunk.area()) {
            is_cursor_at_start = inner
                .sandstorm_frame_chunk
                .draw_line(out, sandstorm_draw_area, is_cursor_at_start, force_redraw)?;
        }

        if let Some(users_draw_area) = area.intersection_with_rect(inner.users_frame_chunk.area()) {
            is_cursor_at_start = inner
                .users_frame_chunk
                .draw_line(out, users_draw_area, is_cursor_at_start, force_redraw)?;
        }

        if let Some(auth_draw_area) = area.intersection_with_rect(inner.auth_frame_chunk.area()) {
            is_cursor_at_start = inner
                .auth_frame_chunk
                .draw_line(out, auth_draw_area, is_cursor_at_start, force_redraw)?;
        }

        if let Some(buffer_draw_area) = area.intersection_with_rect(inner.buffer_frame_chunk.area()) {
            is_cursor_at_start = inner
                .buffer_frame_chunk
                .draw_line(out, buffer_draw_area, is_cursor_at_start, force_redraw)?;
        }

        Ok(is_cursor_at_start)
    }

    fn resize(&mut self, area: Rectangle) {
        self.area = area;
    }
}
