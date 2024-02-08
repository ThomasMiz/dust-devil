use crossterm::event;
use ratatui::{layout::Rect, Frame};

use crate::tui::{
    popups::PopupContent,
    ui_element::{HandleEventStatus, UIElement},
};

pub struct Empty;

impl UIElement for Empty {
    fn resize(&mut self, _area: Rect) {}

    fn render(&mut self, _area: Rect, _buf: &mut Frame) {}

    fn handle_event(&mut self, _event: &event::Event, _is_focused: bool) -> HandleEventStatus {
        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        false
    }

    fn focus_lost(&mut self) {}
}

impl PopupContent for Empty {
    fn begin_resize(&mut self, _width: u16, _height: u16) -> (u16, u16) {
        (0, 0)
    }
}
