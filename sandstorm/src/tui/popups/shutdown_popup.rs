use std::{
    cell::RefCell,
    ops::Deref,
    rc::{Rc, Weak},
};

use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Clear, Widget},
};
use tokio::{
    io::AsyncWrite,
    sync::{oneshot, Notify},
};

use crate::{
    sandstorm::MutexedSandstormRequestManager,
    tui::{
        elements::{
            centered_text::{CenteredText, CenteredTextLine},
            dual_buttons::{DualButtons, DualButtonsHandler},
            focus_cell::FocusCell,
        },
        ui_element::{HandleEventStatus, UIElement},
    },
};

use super::{get_popup_block, CANCEL_NO_KEYS, CANCEL_TITLE, CLOSE_KEY, YES_KEYS, YES_TITLE};

const BACKGROUND_COLOR: Color = Color::Red;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightRed;

const TITLE: &str = "─Shutdown";
const PROMPT_MESSAGE: &str = "Are you sure you want to shut down the server?";
const SHUTDOWN_MESSAGE: &str = "Shutting down...";
const POPUP_WIDTH: u16 = 34;
const PROMPT_STYLE: Style = Style::new();

pub struct ShutdownPopup<W: AsyncWrite + Unpin + 'static> {
    current_size: (u16, u16),
    inner: Rc<RefCell<Inner<W>>>,
    prompt: CenteredText,
    dual_buttons: FocusCell<DualButtons<ButtonHandler<W>>>,
    shutdown_text: CenteredTextLine,
}

struct Inner<W: AsyncWrite + Unpin + 'static> {
    redraw_notify: Rc<Notify>,
    popup_close_sender: Option<oneshot::Sender<()>>,
    manager: Weak<MutexedSandstormRequestManager<W>>,
    shutting_down: bool,
}

struct ButtonHandler<W: AsyncWrite + Unpin + 'static> {
    inner: Weak<RefCell<Inner<W>>>,
}

impl<W: AsyncWrite + Unpin + 'static> ButtonHandler<W> {
    fn new(inner: Weak<RefCell<Inner<W>>>) -> Self {
        Self { inner }
    }
}

impl<W: AsyncWrite + Unpin + 'static> DualButtonsHandler for ButtonHandler<W> {
    fn on_left(&mut self) {
        if let Some(rc) = self.inner.upgrade() {
            rc.deref().borrow_mut().on_yes_selected();
        }
    }

    fn on_right(&mut self) {
        if let Some(rc) = self.inner.upgrade() {
            rc.deref().borrow_mut().close();
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> ShutdownPopup<W> {
    pub fn new(redraw_notify: Rc<Notify>, manager: Weak<MutexedSandstormRequestManager<W>>) -> (Self, oneshot::Receiver<()>) {
        let (close_sender, close_receiver) = oneshot::channel();

        let inner = Rc::new(RefCell::new(Inner {
            redraw_notify: Rc::clone(&redraw_notify),
            popup_close_sender: Some(close_sender),
            manager,
            shutting_down: false,
        }));

        let dual_buttons = DualButtons::new(
            redraw_notify,
            YES_TITLE.into(),
            CANCEL_TITLE.into(),
            YES_KEYS,
            CANCEL_NO_KEYS,
            ButtonHandler::new(Rc::downgrade(&inner)),
            Style::new(),
            Style::new().bg(SELECTED_BACKGROUND_COLOR),
            Style::new(),
            Style::new().bg(SELECTED_BACKGROUND_COLOR),
        );

        let value = Self {
            current_size: (0, 0),
            inner,
            prompt: CenteredText::new(PROMPT_MESSAGE.into(), PROMPT_STYLE),
            dual_buttons: FocusCell::new(dual_buttons),
            shutdown_text: CenteredTextLine::new(SHUTDOWN_MESSAGE.into(), Style::new()),
        };

        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> Inner<W> {
    fn on_yes_selected(&mut self) {
        if self.shutting_down {
            return;
        }

        self.redraw_notify.notify_one();
        self.shutting_down = true;

        if let Some(rc) = self.manager.upgrade() {
            tokio::task::spawn_local(async move {
                let _ = rc.shutdown_fn(|_| ()).await;
            });
        }
    }

    fn close(&mut self) {
        if let Some(close_sender) = self.popup_close_sender.take() {
            let _ = close_sender.send(());
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for ShutdownPopup<W> {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.prompt.resize_if_needed(POPUP_WIDTH - 4);

        self.current_size.0 = area.width.min(POPUP_WIDTH);
        self.current_size.1 = area.height.min(self.prompt.lines_len() + 5);

        let popup_area = Rect::new(
            (area.width - self.current_size.0) / 2,
            (area.height - self.current_size.1) / 2,
            self.current_size.0,
            self.current_size.1,
        );

        Clear.render(popup_area, buf);

        let block = get_popup_block(TITLE, BACKGROUND_COLOR, true);

        let inner_area = block.inner(popup_area);
        block.render(popup_area, buf);

        self.prompt.render(inner_area, buf);

        let buttons_y = inner_area.y + self.prompt.lines_len() + 1;
        if buttons_y < inner_area.bottom() {
            let mut buttons_area = inner_area;
            buttons_area.y = buttons_y;
            buttons_area.height = 1;

            match self.inner.deref().borrow().shutting_down {
                true => self.shutdown_text.render(buttons_area, buf),
                false => self.dual_buttons.render(buttons_area, buf),
            }
        }
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        if self.inner.deref().borrow().shutting_down {
            return HandleEventStatus::Handled;
        }

        if !is_focused {
            return HandleEventStatus::Unhandled;
        }

        if self.dual_buttons.handle_event(event, true) != HandleEventStatus::Unhandled {
            return HandleEventStatus::Handled;
        }

        let key_event = match event {
            event::Event::Key(e) if e.kind != KeyEventKind::Release => e,
            _ => return HandleEventStatus::Unhandled,
        };

        match key_event.code {
            KeyCode::Esc => {
                self.inner.deref().borrow_mut().close();
                HandleEventStatus::Handled
            }
            KeyCode::Char(c) if c.to_ascii_lowercase() == CLOSE_KEY => {
                self.inner.deref().borrow_mut().close();
                HandleEventStatus::Handled
            }
            _ => HandleEventStatus::Unhandled,
        }
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        true
    }

    fn focus_lost(&mut self) {}
}
