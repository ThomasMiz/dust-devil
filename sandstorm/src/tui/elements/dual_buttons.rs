use std::{ops::Deref, rc::Rc};

use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{layout::Rect, style::Style, Frame};
use tokio::sync::Notify;

use crate::tui::{
    text_wrapper::StaticString,
    ui_element::{AutosizeUIElement, HandleEventStatus, PassFocusDirection, UIElement},
};

pub trait DualButtonsHandler {
    fn on_left(&mut self);

    fn on_right(&mut self);
}

pub struct DualButtons<H: DualButtonsHandler> {
    redraw_notify: Rc<Notify>,
    left_str: StaticString,
    right_str: StaticString,
    left_len_chars: u16,
    right_len_chars: u16,
    left_keys: &'static [char],
    right_keys: &'static [char],
    pub handlers: H,
    left_style: Style,
    left_selected_style: Style,
    right_style: Style,
    right_selected_style: Style,
    current_width: u16,
    current_x: u16,
    current_y: u16,
    left_draw_len_chars: u16,
    right_draw_len_chars: u16,
    focused_element: FocusedElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedElement {
    None,
    Left,
    Right,
}

impl FocusedElement {
    pub fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            _ => Self::Left,
        }
    }
}

impl<H: DualButtonsHandler> DualButtons<H> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        redraw_notify: Rc<Notify>,
        left_str: StaticString,
        right_str: StaticString,
        left_keys: &'static [char],
        right_keys: &'static [char],
        handlers: H,
        left_style: Style,
        left_selected_style: Style,
        right_style: Style,
        right_selected_style: Style,
    ) -> Self {
        let left_len_chars = left_str.chars().count().min(u16::MAX as usize) as u16;
        let right_len_chars = right_str.chars().count().min(u16::MAX as usize) as u16;

        Self {
            redraw_notify,
            left_str,
            right_str,
            left_len_chars,
            right_len_chars,
            left_keys,
            right_keys,
            handlers,
            left_style,
            left_selected_style,
            right_style,
            right_selected_style,
            current_width: 0,
            current_x: 0,
            current_y: 0,
            left_draw_len_chars: 0,
            right_draw_len_chars: 0,
            focused_element: FocusedElement::None,
        }
    }

    fn get_focus_position(&self) -> (u16, u16) {
        match self.focused_element {
            FocusedElement::None => (self.current_x + self.current_width / 2, self.current_y),
            FocusedElement::Left => (self.current_x + self.current_width / 4, self.current_y),
            FocusedElement::Right => (self.current_x + self.current_width * 3 / 4, self.current_y),
        }
    }
}

impl<H: DualButtonsHandler> UIElement for DualButtons<H> {
    fn resize(&mut self, area: Rect) {
        self.current_x = area.x;
        self.current_y = area.y;
        if area.width == self.current_width {
            return;
        }

        self.current_width = area.width;

        let available_text_space = self.current_width.saturating_sub(1);
        let min = self.left_len_chars.min(self.right_len_chars).min(available_text_space / 2);
        let mut left_space = min;
        let mut right_space = min;

        let remaining_text_space = available_text_space - min - min;
        if remaining_text_space != 0 {
            if left_space != self.left_len_chars {
                left_space = (left_space + remaining_text_space).min(self.left_len_chars);
            } else if right_space != self.right_len_chars {
                right_space = (right_space + remaining_text_space).min(self.right_len_chars);
            }
        }

        self.left_draw_len_chars = left_space;
        self.right_draw_len_chars = right_space;
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let empty_space = self.current_width - self.left_draw_len_chars - self.right_draw_len_chars;
        let outer_space = empty_space / 3;

        let (left_style, right_style) = match self.focused_element {
            FocusedElement::Left => (self.left_selected_style, self.right_style),
            FocusedElement::Right => (self.left_style, self.right_selected_style),
            _ => (self.left_style, self.right_style),
        };

        let buf = frame.buffer_mut();

        buf.set_stringn(
            area.x + outer_space,
            area.y,
            self.left_str.deref(),
            self.left_draw_len_chars as usize,
            left_style,
        );

        buf.set_stringn(
            area.right() - outer_space - self.right_draw_len_chars,
            area.y,
            self.right_str.deref(),
            self.right_draw_len_chars as usize,
            right_style,
        );
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        let key_event = match event {
            event::Event::Key(key_event) if key_event.kind != KeyEventKind::Release => key_event,
            _ => return HandleEventStatus::Unhandled,
        };

        if let KeyCode::Char(c) = key_event.code {
            if self.left_keys.contains(&c) {
                self.handlers.on_left();
                return HandleEventStatus::Handled;
            }

            if self.right_keys.contains(&c) {
                self.handlers.on_right();
                return HandleEventStatus::Handled;
            }
        }

        if !is_focused {
            return HandleEventStatus::Unhandled;
        }

        let previous_focused_element = self.focused_element;

        let result = match key_event.code {
            KeyCode::Tab => {
                self.focused_element = self.focused_element.other();
                HandleEventStatus::Handled
            }
            KeyCode::Left => {
                if self.focused_element == FocusedElement::Left {
                    HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Left)
                } else {
                    self.focused_element = FocusedElement::Left;
                    HandleEventStatus::Handled
                }
            }
            KeyCode::Right => {
                if self.focused_element == FocusedElement::Right {
                    HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Right)
                } else {
                    self.focused_element = FocusedElement::Right;
                    HandleEventStatus::Handled
                }
            }
            KeyCode::Enter => match self.focused_element {
                FocusedElement::Left => {
                    self.handlers.on_left();
                    HandleEventStatus::Handled
                }
                FocusedElement::Right => {
                    self.handlers.on_right();
                    HandleEventStatus::Handled
                }
                FocusedElement::None => HandleEventStatus::Unhandled,
            },
            KeyCode::Up => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
            KeyCode::Down => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
            KeyCode::Esc => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            _ => HandleEventStatus::Unhandled,
        };

        if self.focused_element != previous_focused_element {
            self.redraw_notify.notify_one();
        }

        result
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.focused_element = if focus_position.0 > self.current_x + self.current_width / 2 {
            FocusedElement::Right
        } else {
            FocusedElement::Left
        };

        self.redraw_notify.notify_one();
        true
    }

    fn focus_lost(&mut self) {
        if self.focused_element != FocusedElement::None {
            self.focused_element = FocusedElement::None;
            self.redraw_notify.notify_one();
        }
    }
}

impl<H: DualButtonsHandler> AutosizeUIElement for DualButtons<H> {
    fn begin_resize(&mut self, width: u16, _height: u16) -> (u16, u16) {
        let optimal_width = self.left_len_chars.saturating_add(self.right_len_chars).saturating_add(3);
        (width.min(optimal_width), 1)
    }
}
