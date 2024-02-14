use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Duration,
};

use crossterm::event;

use dust_devil_core::buffer_size::parse_pretty_buffer_size;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Padding,
    Frame,
};
use tokio::{
    io::AsyncWrite,
    sync::{broadcast, mpsc, oneshot, Notify},
    task::JoinHandle,
};

use crate::{
    sandstorm::MutexedSandstormRequestManager,
    tui::{
        elements::{
            centered_text::{CenteredText, CenteredTextLine},
            dual_buttons::DualButtonsHandler,
            horizontal_split::HorizontalSplit,
            padded::Padded,
            text_entry::{CursorPosition, TextEntry, TextEntryController, TextEntryHandler},
            vertical_split::VerticalSplit,
            OnEnterResult,
        },
        pretty_print::PrettyByteDisplayer,
        ui_element::{AutosizeUIElement, HandleEventStatus, UIElement},
        ui_manager::Popup,
    },
};

use super::{
    message_popup::{MessagePopup, ERROR_POPUP_TITLE, REQUEST_SEND_ERROR_MESSAGE},
    popup_base::PopupBaseController,
    size_constraint::SizeConstraint,
    yes_no_popup::{YesNoControllerInner, YesNoPopup, YesNoPopupController},
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
const INVALID_MESSAGE: &str = "Please enter a valid buffer size";
const SERVER_ERROR_MESSAGE: &str = "The server refused to update the buffer size.";
const NEW_BUFFER_SIZE_LABEL: &str = "New buffer size:";
const ERROR_POPUP_WIDTH: u16 = 40;

pub struct BufferSizePopup<W: AsyncWrite + Unpin + 'static> {
    base: YesNoPopup<Controller<W>, Padded<Content<W>>, ButtonHandler<W>>,
}

struct ControllerInner {
    base: YesNoControllerInner,
    current_task: Option<JoinHandle<()>>,
    is_beeping_red: bool,
    is_doing_request: bool,
    top_message: TopMessage,
}

impl Drop for ControllerInner {
    fn drop(&mut self) {
        if let Some(join_handle) = self.current_task.take() {
            join_handle.abort();
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TopMessage {
    Offer,
    Invalid,
}

impl TopMessage {
    fn to_str(self) -> &'static str {
        match self {
            Self::Offer => OFFER_MESSAGE,
            Self::Invalid => INVALID_MESSAGE,
        }
    }
}

struct Controller<W: AsyncWrite + Unpin + 'static> {
    inner: RefCell<ControllerInner>,
    manager: Weak<MutexedSandstormRequestManager<W>>,
    buffer_size_watch: broadcast::Sender<u32>,
    popup_sender: mpsc::UnboundedSender<Popup>,
}

impl<W: AsyncWrite + Unpin + 'static> Controller<W> {
    fn new(
        redraw_notify: Rc<Notify>,
        has_close_title: bool,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        buffer_size_watch: broadcast::Sender<u32>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (base, close_receiver) = YesNoControllerInner::new(redraw_notify, has_close_title);

        let inner = RefCell::new(ControllerInner {
            base,
            current_task: None,
            is_beeping_red: false,
            is_doing_request: false,
            top_message: TopMessage::Offer,
        });

        let value = Self {
            inner,
            manager,
            buffer_size_watch,
            popup_sender,
        };

        (value, close_receiver)
    }

    fn get_top_message(&self) -> TopMessage {
        self.inner.borrow().top_message
    }

    fn set_top_message(&self, top_message: TopMessage) {
        let mut inner = self.inner.borrow_mut();
        if inner.top_message != top_message {
            inner.top_message = top_message;
            inner.base.base.redraw_notify();
        }
    }

    fn text_entry_beep_red(self: &Rc<Self>, text_controller: &Rc<TextEntryController>) {
        const BEEP_DELAY_MILLIS: u64 = 200;

        let mut inner = self.inner.borrow_mut();
        inner.top_message = TopMessage::Invalid;
        inner.is_beeping_red = true;
        inner.base.base.redraw_notify();
        drop(inner);

        let original_idle_bg = text_controller.get_idle_style().bg;
        let original_typing_bg = text_controller.get_typing_style().bg;

        let self_rc = Rc::clone(self);
        let text_rc = Rc::clone(text_controller);
        let handle = tokio::task::spawn_local(async move {
            for _ in 0..2 {
                text_rc.modify_idle_style(|style| style.bg = Some(Color::Red));
                text_rc.modify_typing_style(|style| style.bg = Some(Color::Red));
                tokio::time::sleep(Duration::from_millis(BEEP_DELAY_MILLIS)).await;
                text_rc.modify_idle_style(|style| style.bg = Some(Color::Reset));
                text_rc.modify_typing_style(|style| style.bg = Some(Color::Reset));
                tokio::time::sleep(Duration::from_millis(BEEP_DELAY_MILLIS)).await;
            }

            text_rc.modify_idle_style(|style| style.bg = original_idle_bg);
            text_rc.modify_typing_style(|style| style.bg = original_typing_bg);

            self_rc.inner.borrow_mut().is_beeping_red = false;
        });

        self.inner.borrow_mut().current_task = Some(handle);
    }

    fn perform_request(self: &Rc<Self>, buffer_size: u32) {
        let mut inner = self.inner.borrow_mut();
        inner.top_message = TopMessage::Offer;
        inner.is_doing_request = true;
        inner.base.set_showing_buttons(false);
        inner.base.base.set_closable(false);
        drop(inner);

        let self_weak = Rc::downgrade(self);
        let handle = tokio::task::spawn_local(async move {
            let manager_rc = match self_weak.upgrade().map(|rc| rc.manager.upgrade()) {
                Some(Some(rc)) => rc,
                _ => return,
            };

            let (response_sender, response_receiver) = oneshot::channel();
            let send_status = manager_rc
                .set_buffer_size_fn(buffer_size, |result| {
                    let _ = response_sender.send(result.0);
                })
                .await;
            drop(manager_rc);

            let result = match send_status {
                Err(_) => Err(false),
                Ok(_) => match response_receiver.await {
                    Ok(status) => Ok(status),
                    Err(_) => Err(true),
                },
            };

            let rc = match self_weak.upgrade() {
                Some(rc) => rc,
                None => return,
            };

            match result {
                Ok(true) => {
                    let _ = rc.buffer_size_watch.send(buffer_size);
                    rc.close_popup();
                }
                _ => {
                    let message_str = match result.is_err() {
                        true => REQUEST_SEND_ERROR_MESSAGE,
                        false => SERVER_ERROR_MESSAGE,
                    };

                    let popup = MessagePopup::empty_error_message(
                        Rc::clone(&rc.inner.borrow().base.base.redraw_notify),
                        ERROR_POPUP_TITLE.into(),
                        message_str.into(),
                        ERROR_POPUP_WIDTH,
                    );

                    let _ = rc.popup_sender.send(popup.into());

                    let mut inner = rc.inner.borrow_mut();
                    inner.is_doing_request = false;
                    inner.base.set_showing_buttons(true);
                    inner.base.base.set_closable(true);
                }
            }
        });

        self.inner.borrow_mut().current_task = Some(handle);
    }

    fn on_yes_selected(self: &Rc<Self>, text_controller: &Rc<TextEntryController>) -> bool {
        let inner = self.inner.borrow();
        if inner.is_beeping_red || inner.is_doing_request {
            return false;
        }
        drop(inner);

        let parse_result = text_controller.with_text(parse_pretty_buffer_size);
        match parse_result {
            Ok(buffer_size) => self.perform_request(buffer_size),
            Err(_) => self.text_entry_beep_red(text_controller),
        }

        parse_result.is_ok()
    }
}

impl<W: AsyncWrite + Unpin + 'static> PopupBaseController for Controller<W> {
    fn redraw_notify(&self) {
        self.inner.borrow_mut().base.base.redraw_notify();
    }

    fn request_resize(&self) {
        self.inner.borrow_mut().base.base.request_resize();
    }

    fn get_resize_requested(&self) -> bool {
        self.inner.borrow_mut().base.base.get_resize_requested()
    }

    fn close_popup(&self) {
        self.inner.borrow_mut().base.base.close_popup();
    }

    fn set_closable(&self, closable: bool) {
        self.inner.borrow_mut().base.base.set_closable(closable);
    }

    fn get_closable(&self) -> bool {
        self.inner.borrow_mut().base.base.get_closable()
    }
}

impl<W: AsyncWrite + Unpin + 'static> YesNoPopupController for Controller<W> {
    fn set_showing_buttons(&self, showing: bool) {
        self.inner.borrow_mut().base.set_showing_buttons(showing);
    }

    fn get_showing_buttons(&self) -> bool {
        self.inner.borrow_mut().base.get_showing_buttons()
    }
}

struct ButtonHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
    text_controller: Rc<TextEntryController>,
}

impl<W: AsyncWrite + Unpin + 'static> ButtonHandler<W> {
    fn new(controller: Rc<Controller<W>>, text_controller: Rc<TextEntryController>) -> Self {
        Self {
            controller,
            text_controller,
        }
    }
}

impl<W: AsyncWrite + Unpin + 'static> DualButtonsHandler for ButtonHandler<W> {
    fn on_left(&mut self) {
        self.controller.on_yes_selected(&self.text_controller);
    }

    fn on_right(&mut self) {
        self.controller.close_popup();
    }
}

struct Content<W: AsyncWrite + Unpin + 'static> {
    base: VerticalSplit<CenteredText, HorizontalSplit<CenteredTextLine, TextEntry<ContentTextHandler<W>>>>,
    current_top_message: TopMessage,
}

impl<W: AsyncWrite + Unpin + 'static> Content<W> {
    fn new(redraw_notify: Rc<Notify>, controller: Rc<Controller<W>>) -> Self {
        let new_size_label = CenteredTextLine::new(NEW_BUFFER_SIZE_LABEL.into(), Style::new());
        let text_entry = TextEntry::new(
            redraw_notify,
            String::new(),
            Style::reset(),
            Style::reset().bg(SELECTED_BACKGROUND_COLOR),
            40,
            ContentTextHandler { controller },
        );

        let text_entry_line = HorizontalSplit::new(new_size_label, text_entry, 0, 1);

        let offer_text = CenteredText::new(OFFER_MESSAGE.into(), Style::new());
        let base = VerticalSplit::new(offer_text, text_entry_line, 0, 1);

        Self {
            base,
            current_top_message: TopMessage::Offer,
        }
    }
}

struct ContentTextHandler<W: AsyncWrite + Unpin + 'static> {
    controller: Rc<Controller<W>>,
}

impl<W: AsyncWrite + Unpin + 'static> TextEntryHandler for ContentTextHandler<W> {
    fn on_enter(&mut self, controller: &Rc<TextEntryController>) -> OnEnterResult {
        match self.controller.on_yes_selected(controller) {
            true => OnEnterResult::PassFocusAway,
            false => OnEnterResult::Handled,
        }
    }

    fn on_char(&mut self, _controller: &Rc<TextEntryController>, c: char, _cursor: &CursorPosition) -> bool {
        c.is_ascii_alphanumeric()
    }

    fn on_text_changed(&mut self, controller: &Rc<TextEntryController>) -> bool {
        let background_color = controller.with_text(|text| match parse_pretty_buffer_size(text) {
            Ok(_) => Color::Green,
            Err(_) => Color::Red,
        });

        controller.modify_typing_style(|style| *style = style.bg(background_color));
        true
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for Content<W> {
    fn resize(&mut self, area: Rect) {
        let controller = &self.base.lower.right.handler.controller;
        controller.set_top_message(TopMessage::Offer);

        self.base.resize(area);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let controller = &self.base.lower.right.handler.controller;
        let top_message = controller.get_top_message();
        if top_message != self.current_top_message {
            self.current_top_message = top_message;
            self.base.upper.modify_text(|text| *text = top_message.to_str().into());
        }

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

impl<W: AsyncWrite + Unpin + 'static> AutosizeUIElement for Content<W> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        self.base.begin_resize(width, height)
    }
}

impl<W: AsyncWrite + Unpin + 'static> BufferSizePopup<W> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        manager: Weak<MutexedSandstormRequestManager<W>>,
        buffer_size: u32,
        buffer_size_watch: broadcast::Sender<u32>,
        popup_sender: mpsc::UnboundedSender<Popup>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (controller, close_receiver) = Controller::new(Rc::clone(&redraw_notify), true, manager, buffer_size_watch, popup_sender);
        let controller = Rc::new(controller);

        let prompt_str = format!("{PROMPT_MESSAGE} {buffer_size} ({}).", PrettyByteDisplayer(buffer_size as usize));

        let content = Padded::new(
            Padding::horizontal(1),
            Content::new(Rc::clone(&redraw_notify), Rc::clone(&controller)),
        );

        let handlers = ButtonHandler::new(Rc::clone(&controller), content.inner.base.lower.right.controller());

        let base = YesNoPopup::new(
            redraw_notify,
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
            SizeConstraint::new().max(POPUP_WIDTH, u16::MAX),
            controller,
            content,
            handlers,
        );

        let value = BufferSizePopup { base };
        (value, close_receiver)
    }
}

impl<W: AsyncWrite + Unpin + 'static> UIElement for BufferSizePopup<W> {
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
