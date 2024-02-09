use std::rc::{Rc, Weak};

use crossterm::event;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Padding,
    Frame,
};
use tokio::{
    io::AsyncWrite,
    sync::{oneshot, Notify},
};

use crate::{
    sandstorm::MutexedSandstormRequestManager,
    tui::{
        elements::{
            centered_text::{CenteredText, CenteredTextLine},
            dual_buttons::DualButtonsHandler,
            horizontal_split::HorizontalSplit,
            padded::Padded,
            text_entry::TextEntry,
            vertical_split::VerticalSplit,
        },
        pretty_print::PrettyByteDisplayer,
        ui_element::{HandleEventStatus, UIElement},
    },
};

use super::{
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
    yes_no_popup::{YesNoPopup, YesNoPopupController, YesNoSimpleController},
    CANCEL_TITLE, CONFIRM_TITLE,
};

const BACKGROUND_COLOR: Color = Color::Magenta;
const SELECTED_BACKGROUND_COLOR: Color = Color::LightMagenta;

const TITLE: &str = "─Set Buffer Size";
const PROMPT_MESSAGE: &str = "The current buffer size for clients is";
const CLOSING_MESSAGE: &str = "Setting buffer size...";
const POPUP_WIDTH: u16 = 40;
const PROMPT_STYLE: Style = Style::new();

const OFFER_MESSAGE: &str = "Do you want to set a new buffer size?";
const NEW_BUFFER_SIZE_LABEL: &str = "New buffer size:";

pub struct BufferSizePopup {
    base:
        YesNoPopup<YesNoSimpleController, Padded<VerticalSplit<CenteredText, HorizontalSplit<CenteredTextLine, TextEntry>>>, ButtonHandler>,
}

struct ButtonHandler {
    controller: Weak<YesNoSimpleController>,
}

impl ButtonHandler {
    fn new(controller: &Rc<YesNoSimpleController>) -> Self {
        Self {
            controller: Rc::downgrade(controller),
        }
    }
}

impl DualButtonsHandler for ButtonHandler {
    fn on_left(&mut self) {
        if let Some(rc) = self.controller.upgrade() {
            rc.set_showing_buttons(false);
            rc.set_closable(false);
        }
    }

    fn on_right(&mut self) {
        if let Some(rc) = self.controller.upgrade() {
            rc.close_popup();
        }
    }
}

impl BufferSizePopup {
    pub fn new<W: AsyncWrite + Unpin + 'static>(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        buffer_size: u32,
    ) -> (Self, oneshot::Receiver<()>) {
        let prompt_str = format!("{PROMPT_MESSAGE} {buffer_size} ({}).", PrettyByteDisplayer(buffer_size as usize));

        let (base, close_receiver) = YesNoPopup::new(
            Rc::clone(&redraw_notify),
            TITLE.into(),
            prompt_str.into(),
            PROMPT_STYLE,
            1,
            CONFIRM_TITLE.into(),
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
            |_controller| {
                let new_size_label = CenteredTextLine::new(NEW_BUFFER_SIZE_LABEL.into(), Style::new());
                let text_entry = TextEntry::new(
                    redraw_notify,
                    String::new(),
                    Style::reset(),
                    Style::reset().bg(SELECTED_BACKGROUND_COLOR),
                    40,
                );

                let text_entry_line = HorizontalSplit::new(new_size_label, text_entry, 0, 1);

                let offer_text = CenteredText::new(OFFER_MESSAGE.into(), Style::new());
                Padded::new(Padding::horizontal(1), VerticalSplit::new(offer_text, text_entry_line, 0, 1))
            },
            |controller| ButtonHandler::new(controller),
        );

        let value = BufferSizePopup { base };
        (value, close_receiver)
    }
}

impl UIElement for BufferSizePopup {
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
