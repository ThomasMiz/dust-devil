use std::{cell::RefCell, ops::Deref, rc::Rc};

use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    style::Color,
    widgets::{Clear, Widget},
};
use tokio::sync::{oneshot, Notify};

use crate::tui::{
    elements::focus_cell::FocusCell,
    text_wrapper::StaticString,
    ui_element::{HandleEventStatus, UIElement},
};

use super::{get_popup_block, ConstrainedPopupContent, PopupContent, CLOSE_KEY};

pub trait PopupBaseController {
    fn redraw_notify(&self);
    fn close_popup(&self);
    fn set_closable(&self, closable: bool);
    fn get_closable(&self) -> bool;
}

pub struct PopupBase<C: PopupBaseController, T: PopupContent> {
    current_size: (u16, u16),
    title: StaticString,
    border_color: Color,
    background_color: Color,
    controller: Rc<C>,
    content: ConstrainedPopupContent<FocusCell<T>>,
}

pub struct PopupBaseSimpleController {
    inner: RefCell<PopupBaseControllerInner>,
}

pub struct PopupBaseControllerInner {
    redraw_notify: Rc<Notify>,
    popup_close_sender: Option<oneshot::Sender<()>>,
    has_close_title: bool,
}

impl PopupBaseControllerInner {
    pub fn redraw_notify(&mut self) {
        self.redraw_notify.notify_one();
    }

    pub fn close(&mut self) {
        if let Some(close_sender) = self.popup_close_sender.take() {
            let _ = close_sender.send(());
        }
    }

    pub fn set_closable(&mut self, closable: bool) {
        if self.has_close_title != closable {
            self.has_close_title = closable;
            self.redraw_notify.notify_one();
        }
    }

    pub fn get_closable(&mut self) -> bool {
        self.has_close_title
    }
}

impl PopupBaseSimpleController {
    pub fn new(inner: PopupBaseControllerInner) -> Self {
        Self {
            inner: RefCell::new(inner),
        }
    }
}

impl PopupBaseController for PopupBaseSimpleController {
    fn redraw_notify(&self) {
        self.inner.borrow_mut().redraw_notify();
    }

    fn close_popup(&self) {
        self.inner.borrow_mut().close();
    }

    fn set_closable(&self, closable: bool) {
        self.inner.borrow_mut().set_closable(closable);
    }

    fn get_closable(&self) -> bool {
        self.inner.borrow_mut().get_closable()
    }
}

impl<C: PopupBaseController, T: PopupContent> PopupBase<C, T> {
    #[allow(clippy::too_many_arguments)]
    pub fn new<CF, TF>(
        redraw_notify: Rc<Notify>,
        title: StaticString,
        border_color: Color,
        background_color: Color,
        has_close_title: bool,
        size_constraint: (u16, u16),
        controller_builder: CF,
        content_builder: TF,
    ) -> (Self, oneshot::Receiver<()>)
    where
        CF: FnOnce(PopupBaseControllerInner) -> C,
        TF: FnOnce(&Rc<C>) -> T,
    {
        let (close_sender, close_receiver) = oneshot::channel();

        let controller_inner = PopupBaseControllerInner {
            redraw_notify,
            popup_close_sender: Some(close_sender),
            has_close_title,
        };

        let controller = Rc::new(controller_builder(controller_inner));
        let content = ConstrainedPopupContent::new(size_constraint, FocusCell::new(content_builder(&controller)));

        let value = Self {
            current_size: (0, 0),
            title,
            border_color,
            background_color,
            controller,
            content,
        };

        (value, close_receiver)
    }
}

impl<C: PopupBaseController, T: PopupContent> UIElement for PopupBase<C, T> {
    fn resize(&mut self, area: Rect) {
        if area.width <= 2 || area.height <= 2 {
            self.current_size = (area.width, area.height);
        } else {
            let (content_width, content_height) = self.content.begin_resize(area.width - 2, area.height - 2);

            self.current_size.0 = area.width.min(content_width + 2);
            self.current_size.1 = area.height.min(content_height + 2);

            let popup_area = Rect::new(
                (area.width - self.current_size.0) / 2,
                (area.height - self.current_size.1) / 2,
                self.current_size.0,
                self.current_size.1,
            );

            let content_area = popup_area.inner(&Margin::new(1, 1));
            self.content.resize(content_area);
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let popup_area = Rect::new(
            (area.width - self.current_size.0) / 2,
            (area.height - self.current_size.1) / 2,
            self.current_size.0,
            self.current_size.1,
        );

        Clear.render(popup_area, buf);

        let has_close_title = self.controller.get_closable();
        let block = get_popup_block(self.title.deref(), self.background_color, self.border_color, has_close_title);

        let content_area = block.inner(popup_area);
        block.render(popup_area, buf);

        self.content.render(content_area, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.content.handle_event(event, is_focused).or_else(|| {
            if !self.controller.get_closable() {
                return HandleEventStatus::Unhandled;
            }

            let key_event = match event {
                event::Event::Key(e) if e.kind != KeyEventKind::Release => e,
                _ => return HandleEventStatus::Unhandled,
            };

            let close = match key_event.code {
                KeyCode::Char(c) => c.to_ascii_lowercase() == CLOSE_KEY,
                KeyCode::Esc => true,
                _ => false,
            };

            if close {
                self.controller.close_popup();
                HandleEventStatus::Handled
            } else {
                HandleEventStatus::Unhandled
            }
        })
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.content.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.content.focus_lost();
    }
}
