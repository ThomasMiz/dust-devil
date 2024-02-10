use std::rc::Rc;

use crossterm::event;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Padding,
    Frame,
};

use crate::tui::{
    elements::{centered_text::CenteredText, padded::Padded, vertical_split::VerticalSplit},
    text_wrapper::StaticString,
    ui_element::{HandleEventStatus, UIElement},
};

use super::{
    popup_base::{PopupBase, PopupBaseController},
    size_constraint::SizeConstraint,
    PopupContent,
};

pub struct PromptPopup<C: PopupBaseController, T: PopupContent> {
    base: PopupBase<C, VerticalSplit<Padded<CenteredText>, T>>,
}

impl<C: PopupBaseController, T: PopupContent> PromptPopup<C, T> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: StaticString,
        prompt_str: StaticString,
        prompt_style: Style,
        prompt_space: u16,
        border_color: Color,
        background_color: Color,
        size_constraint: SizeConstraint,
        controller: Rc<C>,
        content: T,
    ) -> Self {
        let base = PopupBase::new(
            title,
            border_color,
            background_color,
            size_constraint,
            controller,
            VerticalSplit::new(
                Padded::new(Padding::horizontal(1), CenteredText::new(prompt_str, prompt_style)),
                content,
                0,
                prompt_space,
            ),
        );

        PromptPopup { base }
    }
}

impl<C: PopupBaseController, T: PopupContent> UIElement for PromptPopup<C, T> {
    fn resize(&mut self, area: Rect) {
        self.base.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.base.render(area, frame);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.base.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.base.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.base.focus_lost();
    }
}
