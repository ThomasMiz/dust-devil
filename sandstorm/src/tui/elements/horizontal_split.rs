use crossterm::event;
use ratatui::{layout::Rect, Frame};

use crate::tui::ui_element::{AutosizeUIElement, HandleEventStatus, PassFocusDirection, UIElement};

pub struct HorizontalSplit<L: UIElement, R: UIElement> {
    pub left: L,
    pub right: R,
    pub left_width: u16,
    pub space_between: u16,
    current_area: Rect,
    focused_element: FocusedElement,
}

enum FocusedElement {
    Left,
    Right,
}

impl<L: UIElement, R: UIElement> HorizontalSplit<L, R> {
    pub fn new(left: L, right: R, left_width: u16, space_between: u16) -> Self {
        Self {
            left,
            right,
            left_width,
            space_between,
            current_area: Rect::default(),
            focused_element: FocusedElement::Left,
        }
    }
}

impl<L: UIElement, R: UIElement> UIElement for HorizontalSplit<R, L> {
    fn resize(&mut self, area: Rect) {
        self.current_area = area;

        let mut left_area = area;
        left_area.width = left_area.width.min(self.left_width);
        self.left.resize(left_area);

        let left_width_plus_space = self.left_width + self.space_between;

        let mut right_area = area;
        right_area.x += left_width_plus_space;
        right_area.width = right_area.width.saturating_sub(left_width_plus_space);
        self.right.resize(right_area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.current_area = area;

        let mut left_area = area;
        left_area.width = left_area.width.min(self.left_width);
        self.left.render(left_area, frame);

        let left_width_plus_space = self.left_width + self.space_between;

        let mut right_area = area;
        right_area.x += left_width_plus_space;
        right_area.width = right_area.width.saturating_sub(left_width_plus_space);
        self.right.render(right_area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        if !is_focused {
            return self
                .left
                .handle_event(event, false)
                .or_else(|| self.right.handle_event(event, false));
        }

        match self.focused_element {
            FocusedElement::Left => {
                let mut status = self.left.handle_event(event, true);

                match status {
                    HandleEventStatus::Unhandled => {
                        status = self.right.handle_event(event, false);
                    }
                    HandleEventStatus::PassFocus(focus_position, PassFocusDirection::Down | PassFocusDirection::Forward) => {
                        if self.right.receive_focus(focus_position) {
                            self.left.focus_lost();
                            self.focused_element = FocusedElement::Right;
                            status = HandleEventStatus::Handled;
                        }
                    }
                    _ => {}
                }

                status
            }
            FocusedElement::Right => {
                let mut status = self.right.handle_event(event, true);

                match status {
                    HandleEventStatus::Unhandled => {
                        status = self.left.handle_event(event, false);
                    }
                    HandleEventStatus::PassFocus(focus_position, PassFocusDirection::Up | PassFocusDirection::Forward) => {
                        if self.left.receive_focus(focus_position) {
                            self.right.focus_lost();
                            self.focused_element = FocusedElement::Left;
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
        let try_left_first = focus_position.0 <= self.current_area.x + self.left_width + self.space_between / 2;

        if try_left_first && self.left.receive_focus(focus_position) {
            self.focused_element = FocusedElement::Left;
            return true;
        }

        if self.right.receive_focus(focus_position) {
            self.focused_element = FocusedElement::Right;
            return true;
        }

        if !try_left_first && self.left.receive_focus(focus_position) {
            self.focused_element = FocusedElement::Left;
            return true;
        }

        false
    }

    fn focus_lost(&mut self) {
        match self.focused_element {
            FocusedElement::Left => self.left.focus_lost(),
            FocusedElement::Right => self.right.focus_lost(),
        }
    }
}

impl<L: AutosizeUIElement, R: AutosizeUIElement> AutosizeUIElement for HorizontalSplit<L, R> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        let (left_width, left_height) = self.left.begin_resize(width, height);

        self.left_width = left_width;
        let right_available_width = width.saturating_sub(left_width).saturating_sub(self.space_between);

        let (right_width, right_height) = match right_available_width {
            0 => (0, 0),
            _ => self.right.begin_resize(right_available_width, height),
        };

        let optimal_width = left_width.saturating_add(self.space_between).saturating_add(right_width);
        (width.min(optimal_width), left_height.max(right_height))
    }
}
