use std::rc::Rc;

use crossterm::event;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Padding,
    Frame,
};
use tokio::sync::{oneshot, Notify};

use crate::tui::{
    elements::{
        centered_button::{ButtonHandler, CenteredButton},
        empty::Empty,
        padded::Padded,
        vertical_split::VerticalSplit,
        OnEnterResult,
    },
    text_wrapper::StaticString,
    ui_element::{AutosizeUIElement, HandleEventStatus, UIElement},
};

use super::{
    popup_base::{PopupBaseController, PopupBaseSimpleController},
    prompt_popup::PromptPopup,
    size_constraint::SizeConstraint,
};

pub const REQUEST_SEND_ERROR_MESSAGE: &str = "Couldn't send request to server. Is the server still running?";
pub const ERROR_POPUP_TITLE: &str = "─Error";

const BOTTOM_BUTTON_TEXT: &str = "[OK (y)]";
const BOTTOM_BUTTON_SHORTCUT_KEY: char = 'y';

const ERROR_POPUP_BORDER_COLOR: Color = Color::Reset;
const ERROR_POPUP_BACKGROUND_COLOR: Color = Color::Red;
const ERROR_POPUP_SELECTED_COLOR: Color = Color::LightRed;

pub struct MessagePopup<T: AutosizeUIElement> {
    base: PromptPopup<PopupBaseSimpleController, VerticalSplit<T, Padded<CenteredButton<BottomButtonHandler>>>>,
}

struct BottomButtonHandler {
    controller: Rc<PopupBaseSimpleController>,
}

impl BottomButtonHandler {
    fn new(controller: Rc<PopupBaseSimpleController>) -> Self {
        Self { controller }
    }
}

impl ButtonHandler for BottomButtonHandler {
    fn on_pressed(&mut self) -> OnEnterResult {
        self.controller.close_popup();
        OnEnterResult::Handled
    }
}

impl<T: AutosizeUIElement> MessagePopup<T> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        redraw_notify: Rc<Notify>,
        title: StaticString,
        prompt_str: StaticString,
        text_style: Style,
        text_selected_style: Style,
        border_color: Color,
        background_color: Color,
        size_constraint: SizeConstraint,
        content: T,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = PopupBaseSimpleController::new(Rc::clone(&redraw_notify), true);
        let controller = Rc::new(controller);

        let bottom_button = CenteredButton::new(
            redraw_notify,
            BOTTOM_BUTTON_TEXT.into(),
            text_style,
            text_selected_style,
            Some(BOTTOM_BUTTON_SHORTCUT_KEY),
            BottomButtonHandler::new(Rc::clone(&controller)),
        );

        let base = PromptPopup::new(
            title,
            prompt_str,
            text_style,
            1,
            border_color,
            background_color,
            size_constraint,
            controller,
            VerticalSplit::new(content, Padded::new(Padding::new(1, 1, 0, 1), bottom_button), 0, 0),
        );

        let value = Self { base };
        (value, close_receiver)
    }

    pub fn error_message(
        redraw_notify: Rc<Notify>,
        title: StaticString,
        prompt_str: StaticString,
        popup_width: u16,
        content: T,
    ) -> (Self, oneshot::Receiver<()>) {
        Self::new(
            redraw_notify,
            title,
            prompt_str,
            Style::new(),
            Style::new().bg(ERROR_POPUP_SELECTED_COLOR),
            ERROR_POPUP_BORDER_COLOR,
            ERROR_POPUP_BACKGROUND_COLOR,
            SizeConstraint::new().max(popup_width, u16::MAX),
            content,
        )
    }
}

impl MessagePopup<Empty> {
    pub fn empty_error_message(
        redraw_notify: Rc<Notify>,
        title: StaticString,
        prompt_str: StaticString,
        popup_width: u16,
    ) -> (Self, oneshot::Receiver<()>) {
        Self::error_message(redraw_notify, title, prompt_str, popup_width, Empty)
    }
}

impl<T: AutosizeUIElement> UIElement for MessagePopup<T> {
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
