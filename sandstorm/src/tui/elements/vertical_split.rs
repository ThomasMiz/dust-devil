use crossterm::event;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::tui::ui_element::{HandleEventStatus, PassFocusDirection, UIElement};

pub struct VerticalSplit<U: UIElement, L: UIElement> {
    pub upper: U,
    pub lower: L,
    pub upper_height: u16,
    pub space_between: u16,
    current_area: Rect,
    focused_element: FocusedElement,
}

enum FocusedElement {
    Upper,
    Lower,
}

impl FocusedElement {
    fn other(self) -> Self {
        match self {
            Self::Upper => Self::Lower,
            Self::Lower => Self::Upper,
        }
    }
}

impl<U: UIElement, L: UIElement> VerticalSplit<U, L> {
    pub fn new(upper: U, lower: L, upper_height: u16, space_between: u16) -> Self {
        Self {
            upper,
            lower,
            upper_height,
            space_between,
            current_area: Rect::default(),
            focused_element: FocusedElement::Upper,
        }
    }
}

impl<U: UIElement, L: UIElement> UIElement for VerticalSplit<U, L> {
    fn resize(&mut self, area: Rect) {
        let previous_area = self.current_area;
        self.current_area = area;

        if self.current_area.width != previous_area.width
            || self.current_area.height < self.upper_height
            || previous_area.height < self.upper_height
        {
            let mut upper_area = area;
            upper_area.height = upper_area.height.min(self.upper_height);
            self.upper.resize(upper_area);
        }

        let upper_height_plus_space = self.upper_height + self.space_between;
        let mut lower_area = area;
        lower_area.y += upper_height_plus_space;
        lower_area.height = lower_area.height.saturating_sub(upper_height_plus_space);
        self.lower.resize(lower_area);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let mut upper_area = area;
        upper_area.height = upper_area.height.min(self.upper_height);
        self.upper.render(upper_area, buf);

        let upper_height_plus_space = self.upper_height + self.space_between;

        let mut lower_area = area;
        lower_area.y += upper_height_plus_space;
        lower_area.height = lower_area.height.saturating_sub(upper_height_plus_space);
        self.lower.render(lower_area, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        if !is_focused {
            return self
                .upper
                .handle_event(event, false)
                .or_else(|| self.lower.handle_event(event, false));
        }

        match self.focused_element {
            FocusedElement::Upper => {
                let mut status = self.upper.handle_event(event, true);

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
                let mut status = self.lower.handle_event(event, true);

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
        let try_upper_first = focus_position.1 <= self.current_area.y + self.upper_height;

        if try_upper_first && self.upper.receive_focus(focus_position) {
            self.focused_element = FocusedElement::Upper;
            return true;
        }

        if self.lower.receive_focus(focus_position) {
            self.focused_element = FocusedElement::Lower;
            return true;
        }

        if !try_upper_first && self.upper.receive_focus(focus_position) {
            self.focused_element = FocusedElement::Upper;
            return true;
        }

        false
    }

    fn focus_lost(&mut self) {
        match self.focused_element {
            FocusedElement::Upper => self.upper.focus_lost(),
            FocusedElement::Lower => self.lower.focus_lost(),
        }
    }
}
