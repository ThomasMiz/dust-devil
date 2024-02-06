use std::ops::{Deref, DerefMut};

use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{buffer::Buffer, layout::Rect};

use crate::tui::ui_element::{HandleEventStatus, PassFocusDirection, UIElement};

/// A wrapper around another [`UIElement`] that holds onto focus when the inner element requests
/// focus to be passed away with [`PassFocusDirection::Away`]. When that happens, the inner element
/// will be unfocused, but any arrow/tab key event will offer focus back to the inner element.
///
/// When the inner element is unfocused, it will still be offered to handle events (so it can react
/// to key shortcuts), but otherwise keys like ESC are unhandled and may be handled by the
/// containing element. If an arrow/tab key is pressed, focus will be offered back to the inner
/// element.
pub struct FocusCell<I: UIElement> {
    pub inner: I,
    current_area: Rect,
    is_inner_focused: bool,
}

impl<I: UIElement> FocusCell<I> {
    pub fn new(inner: I) -> Self {
        Self {
            inner,
            current_area: Rect::default(),
            is_inner_focused: false,
        }
    }
}

impl<I: UIElement> Deref for FocusCell<I> {
    type Target = I;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<I: UIElement> DerefMut for FocusCell<I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<I: UIElement> UIElement for FocusCell<I> {
    fn resize(&mut self, area: Rect) {
        self.current_area = area;
        self.inner.resize(area);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.inner.render(area, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        let are_buttons_focused = is_focused && self.is_inner_focused;
        match self.inner.handle_event(event, are_buttons_focused) {
            HandleEventStatus::Unhandled => {}
            HandleEventStatus::PassFocus(_focus_position, PassFocusDirection::Away) => {
                self.is_inner_focused = false;
                self.inner.focus_lost();
                return HandleEventStatus::Handled;
            }
            other => return other,
        }

        let key_event = match event {
            event::Event::Key(e) if e.kind != KeyEventKind::Release => e,
            _ => return HandleEventStatus::Unhandled,
        };

        match key_event.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                let focus_position_x = match key_event.code {
                    KeyCode::Left => self.current_area.right(),
                    _ => self.current_area.left(),
                };

                match self.inner.receive_focus((focus_position_x, 0)) {
                    true => {
                        self.is_inner_focused = true;
                        HandleEventStatus::Handled
                    }
                    false => HandleEventStatus::Unhandled,
                }
            }
            _ => HandleEventStatus::Unhandled,
        }
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        if self.inner.receive_focus(focus_position) {
            self.is_inner_focused = true;
        }

        true
    }

    fn focus_lost(&mut self) {
        if self.is_inner_focused {
            self.inner.focus_lost();
            self.is_inner_focused = false;
        }
    }
}
