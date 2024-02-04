use crossterm::event;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::tui::ui_element::{HandleEventStatus, PassFocusDirection, UIElement};

pub struct VerticalSplit<U: UIElement, L: UIElement> {
    pub upper: U,
    pub lower: L,
    pub upper_height: u16,
    current_area: Rect,
    focused_element: FocusedElement,
}

enum FocusedElement {
    None,
    Upper,
    Lower,
}

impl FocusedElement {
    fn other(self) -> Self {
        match self {
            Self::Upper => Self::Lower,
            _ => Self::Upper,
        }
    }
}

impl<U: UIElement, L: UIElement> VerticalSplit<U, L> {
    pub fn new(upper: U, lower: L, upper_height: u16) -> Self {
        Self {
            upper,
            lower,
            upper_height,
            current_area: Rect::default(),
            focused_element: FocusedElement::None,
        }
    }
}

impl<U: UIElement, L: UIElement> UIElement for VerticalSplit<U, L> {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let mut upper_area = area;
        upper_area.height = upper_area.height.min(self.upper_height);
        self.upper.render(upper_area, buf);

        let mut lower_area = area;
        lower_area.y += self.upper_height;
        lower_area.height = lower_area.height.saturating_sub(self.upper_height);
        self.lower.render(lower_area, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        match self.focused_element {
            FocusedElement::None => self
                .upper
                .handle_event(event, false)
                .or_else(|| self.lower.handle_event(event, false)),
            FocusedElement::Upper => {
                let mut status = self.upper.handle_event(event, is_focused);

                match status {
                    HandleEventStatus::Unhandled => {
                        status = self.lower.handle_event(event, false);
                    }
                    HandleEventStatus::PassFocus(focus_position, PassFocusDirection::Down | PassFocusDirection::Forward) => {
                        if self.lower.receive_focus(focus_position) {
                            self.focused_element = FocusedElement::Lower;
                            status = HandleEventStatus::Handled;
                        }
                    }
                    _ => {}
                }

                status
            }
            FocusedElement::Lower => {
                let mut status = self.lower.handle_event(event, is_focused);

                match status {
                    HandleEventStatus::Unhandled => {
                        status = self.upper.handle_event(event, false);
                    }
                    HandleEventStatus::PassFocus(focus_position, PassFocusDirection::Up | PassFocusDirection::Forward) => {
                        if self.upper.receive_focus(focus_position) {
                            self.focused_element = FocusedElement::Upper;
                            status = HandleEventStatus::Handled;
                        }
                    }
                    _ => {}
                }

                status
            }
        }
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        if focus_position.1 <= self.current_area.y + self.upper_height {
            self.upper.receive_focus(focus_position) || self.lower.receive_focus(focus_position)
        } else {
            self.lower.receive_focus(focus_position) || self.upper.receive_focus(focus_position)
        }
    }

    fn focus_lost(&mut self) {
        match self.focused_element {
            FocusedElement::None => {}
            FocusedElement::Upper => self.upper.focus_lost(),
            FocusedElement::Lower => self.lower.focus_lost(),
        }

        self.focused_element = FocusedElement::None;
    }
}
