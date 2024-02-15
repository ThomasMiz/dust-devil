use std::{ops::Deref, rc::Rc};

use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{layout::Rect, style::Style, Frame};
use tokio::sync::Notify;

use crate::tui::{
    text_wrapper::StaticString,
    ui_element::{AutosizeUIElement, HandleEventStatus, PassFocusDirection, UIElement},
};

use super::OnEnterResult;

pub trait ButtonHandler {
    fn on_pressed(&mut self) -> OnEnterResult;
}

pub struct CenteredButton<H: ButtonHandler> {
    redraw_notify: Rc<Notify>,
    text: StaticString,
    text_len_chars: u16,
    idle_style: Style,
    focused_style: Style,
    shortcut_key: Option<char>,
    pub handler: H,
    is_focused: bool,
    current_position: (u16, u16),
    current_width: u16,
}

impl<H: ButtonHandler> CenteredButton<H> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        text: StaticString,
        idle_style: Style,
        focused_style: Style,
        shortcut_key: Option<char>,
        handler: H,
    ) -> Self {
        let text_len_chars = text.deref().chars().count().min(u16::MAX as usize) as u16;
        let shortcut_key = shortcut_key.map(|c| c.to_ascii_lowercase());

        Self {
            redraw_notify,
            text,
            text_len_chars,
            idle_style,
            focused_style,
            shortcut_key,
            handler,
            is_focused: false,
            current_position: (0, 0),
            current_width: 0,
        }
    }

    fn get_focus_position(&self) -> (u16, u16) {
        self.current_position
    }
}

impl<H: ButtonHandler> UIElement for CenteredButton<H> {
    fn resize(&mut self, area: Rect) {
        self.current_position = (area.x, area.y);
        self.current_width = area.width;
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let x = area.x + area.width.saturating_sub(self.text_len_chars) / 2;

        let style = match self.is_focused {
            true => self.focused_style,
            false => self.idle_style,
        };

        let width = area.width as usize;
        frame.buffer_mut().set_stringn(x, area.y, self.text.deref(), width, style);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        let key_event = match event {
            event::Event::Key(e) if e.kind != KeyEventKind::Release => e,
            _ => return HandleEventStatus::Unhandled,
        };

        if let KeyCode::Char(c) = key_event.code {
            if self.shortcut_key.is_some_and(|sk| sk == c.to_ascii_lowercase()) {
                self.handler.on_pressed();
                return HandleEventStatus::Handled;
            }

            return HandleEventStatus::Unhandled;
        }

        if !is_focused {
            return HandleEventStatus::Unhandled;
        }

        match key_event.code {
            KeyCode::Enter => match self.handler.on_pressed() {
                OnEnterResult::Handled => HandleEventStatus::Handled,
                OnEnterResult::Unhandled => HandleEventStatus::Unhandled,
                OnEnterResult::PassFocusAway => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            },
            KeyCode::Left => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Left),
            KeyCode::Right => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Right),
            KeyCode::Up => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
            KeyCode::Down => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
            KeyCode::Tab => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Forward),
            KeyCode::Esc => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            _ => HandleEventStatus::Unhandled,
        }
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        self.is_focused = true;
        self.redraw_notify.notify_one();
        true
    }

    fn focus_lost(&mut self) {
        self.is_focused = false;
        self.redraw_notify.notify_one();
    }
}

impl<H: ButtonHandler> AutosizeUIElement for CenteredButton<H> {
    fn begin_resize(&mut self, width: u16, _height: u16) -> (u16, u16) {
        (width.min(self.text_len_chars), 1)
    }
}
