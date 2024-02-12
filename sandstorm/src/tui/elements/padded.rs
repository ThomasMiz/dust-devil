use crossterm::event;
use ratatui::{layout::Rect, widgets::Padding, Frame};

use crate::tui::ui_element::{AutosizeUIElement, HandleEventStatus, UIElement};

pub struct Padded<T: UIElement> {
    pub padding: Padding,
    pub inner: T,
}

impl<T: UIElement> Padded<T> {
    pub fn new(padding: Padding, inner: T) -> Self {
        Self { padding, inner }
    }

    pub fn inner_area(&self, area: Rect) -> Rect {
        Rect::new(
            area.x + self.padding.left,
            area.y + self.padding.top,
            area.width.saturating_sub(self.padding.right + self.padding.left),
            area.height.saturating_sub(self.padding.top + self.padding.bottom),
        )
    }
}

impl<T: UIElement> UIElement for Padded<T> {
    fn resize(&mut self, area: Rect) {
        self.inner.resize(self.inner_area(area));
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.inner.render(self.inner_area(area), frame);
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

impl<T: AutosizeUIElement> AutosizeUIElement for Padded<T> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        let width = width.saturating_sub(self.padding.left).saturating_sub(self.padding.right);
        let height = height.saturating_sub(self.padding.top).saturating_sub(self.padding.bottom);

        let (width, height) = self.inner.begin_resize(width, height);

        let width = width.saturating_add(self.padding.left).saturating_add(self.padding.right);
        let height = height.saturating_add(self.padding.top).saturating_add(self.padding.bottom);

        (width, height)
    }
}
