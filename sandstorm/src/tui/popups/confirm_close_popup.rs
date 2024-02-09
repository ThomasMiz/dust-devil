use std::rc::{Rc, Weak};

use crossterm::event;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    Frame,
};
use tokio::sync::{oneshot, Notify};

use crate::tui::{
    elements::{dual_buttons::DualButtonsHandler, empty::Empty},
    ui_element::{HandleEventStatus, UIElement},
};

use super::{
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
    yes_no_popup::{YesNoPopup, YesNoPopupController, YesNoSimpleController},
    CANCEL_TITLE, YES_TITLE,
};

const BACKGROUND_COLOR: Color = Color::Blue;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightBlue;

const TITLE: &str = "─Close";
const PROMPT_MESSAGE: &str = "Are you sure you want to close this terminal?";
const CLOSING_MESSAGE: &str = "Closing...";
const POPUP_WIDTH: u16 = 32;
const PROMPT_STYLE: Style = Style::new();

pub struct ConfirmClosePopup {
    base: YesNoPopup<YesNoSimpleController, Empty, ButtonHandler>,
}

struct ButtonHandler {
    controller: Weak<YesNoSimpleController>,
    shutdown_notify: Rc<Notify>,
}

impl ButtonHandler {
    fn new(controller: &Rc<YesNoSimpleController>, shutdown_notify: Rc<Notify>) -> Self {
        Self {
            controller: Rc::downgrade(controller),
            shutdown_notify,
        }
    }
}

impl DualButtonsHandler for ButtonHandler {
    fn on_left(&mut self) {
        if let Some(rc) = self.controller.upgrade() {
            rc.set_showing_buttons(false);
            rc.set_closable(false);
            self.shutdown_notify.notify_one();
        }
    }

    fn on_right(&mut self) {
        if let Some(rc) = self.controller.upgrade() {
            rc.close_popup();
        }
    }
}

impl ConfirmClosePopup {
    pub fn new(redraw_notify: Rc<Notify>, shutdown_notify: Rc<Notify>) -> (Self, oneshot::Receiver<()>) {
        let (base, close_receiver) = YesNoPopup::new(
            redraw_notify,
            TITLE.into(),
            PROMPT_MESSAGE.into(),
            PROMPT_STYLE,
            0,
            YES_TITLE.into(),
            CANCEL_TITLE.into(),
            Style::new(),
            Style::new().bg(SELECTED_BACKGROUND_COLOR),
            Style::new(),
            Style::new().bg(SELECTED_BACKGROUND_COLOR),
            CLOSING_MESSAGE.into(),
            Style::new(),
            Color::Reset,
            BACKGROUND_COLOR,
            true,
            SizeConstraint::new().max(POPUP_WIDTH, u16::MAX),
            YesNoSimpleController::new,
            |_controller| Empty,
            |controller| ButtonHandler::new(controller, shutdown_notify),
        );

        let value = ConfirmClosePopup { base };
        (value, close_receiver)
    }
}

impl UIElement for ConfirmClosePopup {
    fn resize(&mut self, area: Rect) {
        self.base.resize(area)
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
