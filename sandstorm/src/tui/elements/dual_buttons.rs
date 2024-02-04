use std::rc::Rc;

use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use tokio::sync::Notify;

use crate::tui::ui_element::{HandleEventStatus, PassFocusDirection, UIElement};

pub struct DualButtons<'a> {
    redraw_notify: Rc<Notify>,
    left_str: &'a str,
    right_str: &'a str,
    left_keys: &'a [char],
    right_keys: &'a [char],
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    None,
    Left,
    Right,
}

pub enum DualButtonsEventStatus {
    Handled(Button),
    Unhandled,
    PassFocus((u16, u16), PassFocusDirection),
}

impl From<DualButtonsEventStatus> for HandleEventStatus {
    fn from(value: DualButtonsEventStatus) -> Self {
        match value {
            DualButtonsEventStatus::Handled(_) => HandleEventStatus::Handled,
            DualButtonsEventStatus::Unhandled => HandleEventStatus::Unhandled,
            DualButtonsEventStatus::PassFocus(focus_position, direction) => HandleEventStatus::PassFocus(focus_position, direction),
        }
    }
}

impl<'a> DualButtons<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        redraw_notify: Rc<Notify>,
        left_str: &'a str,
        right_str: &'a str,
        left_keys: &'a [char],
        right_keys: &'a [char],
        left_style: Style,
        left_selected_style: Style,
        right_style: Style,
        right_selected_style: Style,
    ) -> Self {
        Self {
            redraw_notify,
            left_str,
            right_str,
            left_keys,
            right_keys,
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

    pub fn resize_if_needed(&mut self, area: Rect) {
        self.current_x = area.x;
        self.current_y = area.y;
        if area.width == self.current_width {
            return;
        }

        self.current_width = area.width;

        let left_len_chars = self.left_str.chars().count().min(u16::MAX as usize) as u16;
        let right_len_chars = self.right_str.chars().count().min(u16::MAX as usize) as u16;

        let mut available_text_space = self.current_width.saturating_sub(1);
        let mut left_space = 0;
        let mut right_space = 0;

        while available_text_space != 0 && left_space != left_len_chars && right_space != right_len_chars {
            if left_space >= right_space {
                left_space += 1;
            } else {
                right_space += 1;
            }

            available_text_space -= 1;
        }

        if left_space != left_len_chars {
            left_space = (left_space + available_text_space).min(left_len_chars);
        } else if right_space != right_len_chars {
            right_space = (right_space + available_text_space).min(right_len_chars);
        }

        self.left_draw_len_chars = left_space;
        self.right_draw_len_chars = right_space;
    }

    pub fn handle_event_with_result(&mut self, event: &event::Event, is_focused: bool) -> DualButtonsEventStatus {
        let key_event = match event {
            event::Event::Key(key_event) if key_event.kind != KeyEventKind::Release => key_event,
            _ => return DualButtonsEventStatus::Unhandled,
        };

        if let KeyCode::Char(c) = key_event.code {
            if self.left_keys.contains(&c) {
                return DualButtonsEventStatus::Handled(Button::Left);
            }

            if self.right_keys.contains(&c) {
                return DualButtonsEventStatus::Handled(Button::Right);
            }
        }

        if !is_focused {
            return DualButtonsEventStatus::Unhandled;
        }

        let previous_focused_element = self.focused_element;

        let result = match key_event.code {
            KeyCode::Tab => {
                self.focused_element = self.focused_element.other();
                DualButtonsEventStatus::Handled(Button::None)
            }
            KeyCode::Left => {
                if self.focused_element == FocusedElement::Left {
                    DualButtonsEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Left)
                } else {
                    self.focused_element = FocusedElement::Left;
                    DualButtonsEventStatus::Handled(Button::None)
                }
            }
            KeyCode::Right => {
                if self.focused_element == FocusedElement::Right {
                    DualButtonsEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Right)
                } else {
                    self.focused_element = FocusedElement::Right;
                    DualButtonsEventStatus::Handled(Button::None)
                }
            }
            KeyCode::Enter => match self.focused_element {
                FocusedElement::Left => DualButtonsEventStatus::Handled(Button::Left),
                FocusedElement::Right => DualButtonsEventStatus::Handled(Button::Right),
                FocusedElement::None => DualButtonsEventStatus::Unhandled,
            },
            KeyCode::Up => DualButtonsEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
            KeyCode::Down => DualButtonsEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
            KeyCode::Esc => DualButtonsEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            _ => DualButtonsEventStatus::Unhandled,
        };

        if self.focused_element != previous_focused_element {
            self.redraw_notify.notify_one();
        }

        result
    }
}

impl<'a> UIElement for DualButtons<'a> {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.resize_if_needed(area);

        let empty_space = self.current_width - self.left_draw_len_chars - self.right_draw_len_chars;
        let outer_space = empty_space / 3;

        let (left_style, right_style) = match self.focused_element {
            FocusedElement::Left => (self.left_selected_style, self.right_style),
            FocusedElement::Right => (self.left_style, self.right_selected_style),
            _ => (self.left_style, self.right_style),
        };

        buf.set_stringn(
            area.x + outer_space,
            area.y,
            self.left_str,
            self.left_draw_len_chars as usize,
            left_style,
        );

        buf.set_stringn(
            area.right() - outer_space - self.right_draw_len_chars,
            area.y,
            self.right_str,
            self.right_draw_len_chars as usize,
            right_style,
        );
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.handle_event_with_result(event, is_focused).into()
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
