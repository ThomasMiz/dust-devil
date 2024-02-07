use std::rc::Rc;

use crossterm::event;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Padding,
};
use tokio::sync::{oneshot, Notify};

use crate::tui::{
    elements::{centered_text::CenteredText, padded::Padded, vertical_split::VerticalSplit},
    text_wrapper::StaticString,
    ui_element::{HandleEventStatus, UIElement},
};

use super::{
    popup_base::{PopupBase, PopupBaseController, PopupBaseControllerInner},
    size_constraint::SizeConstraint,
    PopupContent,
};

pub struct PromptPopup<C: PopupBaseController, T: PopupContent> {
    base: PopupBase<C, VerticalSplit<Padded<CenteredText>, T>>,
}

impl<C: PopupBaseController, T: PopupContent> PromptPopup<C, T> {
    #[allow(clippy::too_many_arguments)]
    pub fn new<CF, TF>(
        redraw_notify: Rc<Notify>,
        title: StaticString,
        prompt_str: StaticString,
        prompt_style: Style,
        prompt_space: u16,
        border_color: Color,
        background_color: Color,
        has_close_title: bool,
        size_constraint: SizeConstraint,
        controller_builder: CF,
        content_builder: TF,
    ) -> (Self, oneshot::Receiver<()>)
    where
        CF: FnOnce(PopupBaseControllerInner) -> C,
        TF: FnOnce(&Rc<C>) -> T,
    {
        let (base, receiver) = PopupBase::new(
            redraw_notify,
            title,
            border_color,
            background_color,
            has_close_title,
            size_constraint,
            controller_builder,
            |controller| {
                VerticalSplit::new(
                    Padded::new(Padding::horizontal(1), CenteredText::new(prompt_str, prompt_style)),
                    content_builder(controller),
                    0,
                    prompt_space,
                )
            },
        );

        let value = PromptPopup { base };
        (value, receiver)
    }
}

impl<C: PopupBaseController, T: PopupContent> UIElement for PromptPopup<C, T> {
    fn resize(&mut self, area: Rect) {
        self.base.resize(area);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.base.render(area, buf);
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
