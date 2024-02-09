use std::rc::{Rc, Weak};

use crossterm::event;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    Frame,
};
use tokio::{
    io::AsyncWrite,
    sync::{oneshot, Notify},
};

use crate::{
    sandstorm::MutexedSandstormRequestManager,
    tui::{
        elements::{dual_buttons::DualButtonsHandler, empty::Empty},
        ui_element::{HandleEventStatus, UIElement},
    },
};

use super::{
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
    yes_no_popup::{YesNoPopup, YesNoPopupController, YesNoSimpleController},
    CANCEL_TITLE, YES_TITLE,
};

const BACKGROUND_COLOR: Color = Color::Red;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightRed;

const TITLE: &str = "─Shutdown";
const PROMPT_MESSAGE: &str = "Are you sure you want to shut down the server?";
const SHUTDOWN_MESSAGE: &str = "Shutting down...";
const POPUP_WIDTH: u16 = 32;
const PROMPT_STYLE: Style = Style::new();

pub struct ShutdownPopup<W: AsyncWrite + Unpin + 'static> {
    base: YesNoPopup<YesNoSimpleController, Empty, ButtonHandler<W>>,
}

struct ButtonHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Weak<YesNoSimpleController>,
    manager: Weak<MutexedSandstormRequestManager<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> ButtonHandler<W> {
    fn new(controller: &Rc<YesNoSimpleController>, manager: Weak<MutexedSandstormRequestManager<W>>) -> Self {
        Self {
            controller: Rc::downgrade(controller),
            manager,
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> DualButtonsHandler for ButtonHandler<W> {
    fn on_left(&mut self) {
        if let Some(rc) = self.manager.upgrade() {
            tokio::task::spawn_local(async move {
                let _ = rc.shutdown_fn(|_| ()).await;
            });

            if let Some(rc) = self.controller.upgrade() {
                rc.set_showing_buttons(false);
                rc.set_closable(false);
            }
        }
    }

    fn on_right(&mut self) {
        if let Some(rc) = self.controller.upgrade() {
            rc.close_popup();
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> ShutdownPopup<W> {
    pub fn new(redraw_notify: Rc<Notify>, manager: Weak<MutexedSandstormRequestManager<W>>) -> (Self, oneshot::Receiver<()>) {
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
            SHUTDOWN_MESSAGE.into(),
            Style::new(),
            Color::Reset,
            BACKGROUND_COLOR,
            true,
            SizeConstraint::new().max(POPUP_WIDTH, u16::MAX),
            YesNoSimpleController::new,
            |_controller| Empty,
            |controller| ButtonHandler::new(controller, manager),
        );

        let value = ShutdownPopup { base };
        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for ShutdownPopup<W> {
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
