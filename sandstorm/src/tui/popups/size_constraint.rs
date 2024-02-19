use crossterm::event;
use ratatui::{layout::Rect, Frame};

use crate::tui::ui_element::{AutosizeUIElement, HandleEventStatus, UIElement};

pub struct SizeConstraint {
    max: (u16, u16),
}

impl SizeConstraint {
    pub const fn empty() -> Self {
        Self { max: (u16::MAX, u16::MAX) }
    }

    pub const fn new(max_width: u16, max_height: u16) -> Self {
        Self {
            max: (max_width, max_height),
        }
    }
}

impl Default for SizeConstraint {
    fn default() -> Self {
        Self::empty()
    }
}

pub struct ConstrainedPopupContent<T: AutosizeUIElement> {
    pub size_constraint: SizeConstraint,
    pub inner: T,
}

impl<T: AutosizeUIElement> ConstrainedPopupContent<T> {
    pub fn new(size_constraint: SizeConstraint, inner: T) -> Self {
        Self { size_constraint, inner }
    }
}

impl<T: AutosizeUIElement> UIElement for ConstrainedPopupContent<T> {
    fn resize(&mut self, area: Rect) {
        self.inner.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.inner.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.inner.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.inner.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.inner.focus_lost();
    }
}

impl<T: AutosizeUIElement> AutosizeUIElement for ConstrainedPopupContent<T> {
    fn begin_resize(&mut self, mut width: u16, mut height: u16) -> (u16, u16) {
        width = width.min(self.size_constraint.max.0);
        height = height.min(self.size_constraint.max.1);

        let (mut width, mut height) = self.inner.begin_resize(width, height);

        width = width.min(self.size_constraint.max.0);
        height = height.min(self.size_constraint.max.1);

        (width, height)
    }
}
