use std::rc::Rc;

use crossterm::event;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::Padding,
    Frame,
};
use tokio::sync::{oneshot, Notify};

use crate::tui::{
    elements::{padded::Padded, text::Text},
    text_wrapper::StaticString,
    ui_element::{AutosizeUIElement, HandleEventStatus, UIElement},
};

use super::{
    popup_base::{PopupBase, PopupBaseController, PopupBaseControllerInner},
    size_constraint::SizeConstraint,
};

pub trait LoadingPopupController: PopupBaseController {
    fn set_is_loading(&self, loading: bool);
    fn get_is_loading(&self) -> bool;
    fn set_new_loading_text(&self, text: StaticString);
    fn get_new_loading_text(&self) -> Option<StaticString>;
}

pub struct LoadingPopupControllerInner {
    pub base: PopupBaseControllerInner,
    is_loading: bool,
    new_loading_text: Option<StaticString>,
}

impl LoadingPopupControllerInner {
    pub fn new(redraw_notify: Rc<Notify>, has_close_title: bool, is_loading: bool) -> (Self, oneshot::Receiver<()>) {
        let (base, close_receiver) = PopupBaseControllerInner::new(redraw_notify, has_close_title);

        let value = Self {
            base,
            is_loading,
            new_loading_text: None,
        };

        (value, close_receiver)
    }

    pub fn set_is_loading(&mut self, loading: bool) {
        if self.is_loading != loading {
            self.is_loading = loading;
            self.base.request_resize();
        }
    }

    pub fn get_is_loading(&self) -> bool {
        self.is_loading
    }

    pub fn set_new_loading_text(&mut self, text: StaticString) {
        self.base.request_resize();
        self.new_loading_text = Some(text);
    }

    pub fn get_new_loading_text(&mut self) -> Option<StaticString> {
        self.new_loading_text.take()
    }
}

pub struct LoadingPopup<C: LoadingPopupController, T: AutosizeUIElement> {
    base: PopupBase<C, LoadingContent<C, T>>,
}

impl<C: LoadingPopupController, T: AutosizeUIElement> LoadingPopup<C, T> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: StaticString,
        loading_str: StaticString,
        loading_style: Style,
        border_color: Color,
        background_color: Color,
        size_constraint: SizeConstraint,
        controller: Rc<C>,
        content: T,
    ) -> Self {
        let loading_content = LoadingContent {
            controller: Rc::clone(&controller),
            loading_text: Padded::new(Padding::horizontal(1), Text::new(loading_str, loading_style, Alignment::Center)),
            content,
        };

        let base = PopupBase::new(title, border_color, background_color, size_constraint, controller, loading_content);
        LoadingPopup { base }
    }
}

struct LoadingContent<C: LoadingPopupController, T: AutosizeUIElement> {
    controller: Rc<C>,
    loading_text: Padded<Text>,
    content: T,
}

impl<C: LoadingPopupController, T: AutosizeUIElement> UIElement for LoadingContent<C, T> {
    fn resize(&mut self, area: Rect) {
        match self.controller.get_is_loading() {
            true => self.loading_text.resize(area),
            false => self.content.resize(area),
        }
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        match self.controller.get_is_loading() {
            true => self.loading_text.render(area, frame),
            false => self.content.render(area, frame),
        }
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        match self.controller.get_is_loading() {
            true => self.loading_text.handle_event(event, is_focused),
            false => self.content.handle_event(event, is_focused),
        }
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        match self.controller.get_is_loading() {
            true => self.loading_text.receive_focus(focus_position),
            false => self.content.receive_focus(focus_position),
        }
    }

    fn focus_lost(&mut self) {
        match self.controller.get_is_loading() {
            true => self.loading_text.focus_lost(),
            false => self.content.focus_lost(),
        }
    }
}

impl<C: LoadingPopupController, T: AutosizeUIElement> AutosizeUIElement for LoadingContent<C, T> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        match self.controller.get_is_loading() {
            true => {
                if let Some(new_loading_text) = self.controller.get_new_loading_text() {
                    let style = self.loading_text.inner.style();
                    self.loading_text.inner = Text::new(new_loading_text, style, Alignment::Center);
                }

                self.loading_text.begin_resize(width, height)
            }
            false => self.content.begin_resize(width, height),
        }
    }
}

impl<C: LoadingPopupController, T: AutosizeUIElement> UIElement for LoadingPopup<C, T> {
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
