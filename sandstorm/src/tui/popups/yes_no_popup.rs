use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crossterm::event;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Padding,
};
use tokio::sync::{oneshot, Notify};

use crate::tui::{
    elements::{
        centered_text::CenteredTextLine,
        dual_buttons::{DualButtons, DualButtonsHandler},
        padded::Padded,
        vertical_split::VerticalSplit,
    },
    text_wrapper::StaticString,
    ui_element::{HandleEventStatus, UIElement},
};

use super::{
    popup_base::{PopupBaseController, PopupBaseControllerInner},
    prompt_popup::PromptPopup,
    size_constraint::SizeConstraint,
    PopupContent, CANCEL_NO_KEYS, YES_KEYS,
};

pub trait YesNoPopupController: PopupBaseController {
    fn set_showing_buttons(&self, showing: bool);
    fn get_showing_buttons(&self) -> bool;
}

pub struct YesNoPopup<C: YesNoPopupController, T: PopupContent, H: DualButtonsHandler> {
    base: PromptPopup<C, VerticalSplit<T, Padded<ButtonsOrTextLine<C, H>>>>,
}

struct ButtonsOrTextLine<C: YesNoPopupController, H: DualButtonsHandler> {
    controller: Rc<C>,
    buttons: DualButtons<H>,
    alternative_text: CenteredTextLine,
}

impl<C: YesNoPopupController, H: DualButtonsHandler> UIElement for ButtonsOrTextLine<C, H> {
    fn resize(&mut self, area: Rect) {
        self.buttons.resize(area);
        self.alternative_text.resize(area);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        match self.controller.get_showing_buttons() {
            true => self.buttons.render(area, buf),
            false => self.alternative_text.render(area, buf),
        }
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        match self.controller.get_showing_buttons() {
            true => self.buttons.handle_event(event, is_focused),
            false => self.alternative_text.handle_event(event, is_focused),
        }
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.controller.get_showing_buttons() && self.buttons.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        if self.controller.get_showing_buttons() {
            self.buttons.focus_lost();
        }
    }
}

impl<C: YesNoPopupController, H: DualButtonsHandler> PopupContent for ButtonsOrTextLine<C, H> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        self.buttons.begin_resize(width, height)
    }
}

pub struct YesNoControllerInner {
    pub base: PopupBaseControllerInner,
    is_showing_buttons: bool,
}

impl YesNoControllerInner {
    pub fn set_showing_buttons(&mut self, showing: bool) {
        if self.is_showing_buttons != showing {
            self.is_showing_buttons = showing;
            self.base.redraw_notify();
        }
    }

    pub fn get_showing_buttons(&mut self) -> bool {
        self.is_showing_buttons
    }
}

pub struct YesNoSimpleController {
    inner: RefCell<YesNoControllerInner>,
}

impl YesNoSimpleController {
    pub fn new(inner: YesNoControllerInner) -> Self {
        Self {
            inner: RefCell::new(inner),
        }
    }
}

impl PopupBaseController for YesNoSimpleController {
    fn redraw_notify(&self) {
        self.inner.borrow_mut().base.redraw_notify();
    }

    fn close_popup(&self) {
        self.inner.borrow_mut().base.close_popup();
    }

    fn set_closable(&self, closable: bool) {
        self.inner.borrow_mut().base.set_closable(closable);
    }

    fn get_closable(&self) -> bool {
        self.inner.borrow_mut().base.get_closable()
    }
}

impl YesNoPopupController for YesNoSimpleController {
    fn set_showing_buttons(&self, showing: bool) {
        self.inner.borrow_mut().set_showing_buttons(showing);
    }

    fn get_showing_buttons(&self) -> bool {
        self.inner.borrow_mut().get_showing_buttons()
    }
}

impl<C: YesNoPopupController, T: PopupContent, H: DualButtonsHandler> YesNoPopup<C, T, H> {
    #[allow(clippy::too_many_arguments)]
    pub fn new<CF, TF, HF>(
        redraw_notify: Rc<Notify>,
        title: StaticString,
        prompt_str: StaticString,
        prompt_style: Style,
        prompt_space: u16,
        yes_text: StaticString,
        no_text: StaticString,
        yes_style: Style,
        yes_selected_style: Style,
        no_style: Style,
        no_selected_style: Style,
        alternative_text: StaticString,
        alternative_text_style: Style,
        border_color: Color,
        background_color: Color,
        has_close_title: bool,
        size_constraint: SizeConstraint,
        controller_builder: CF,
        content_builder: TF,
        handler_builder: HF,
    ) -> (Self, oneshot::Receiver<()>)
    where
        CF: FnOnce(YesNoControllerInner) -> C,
        TF: FnOnce(&Rc<C>) -> T,
        HF: FnOnce(&Rc<C>) -> H,
    {
        // base: PromptPopup<C, VerticalSplit<T, Padded<ButtonsOrTextLine<C, H>>>>,

        let (base, receiver) = PromptPopup::new(
            Rc::clone(&redraw_notify),
            title,
            prompt_str,
            prompt_style,
            prompt_space,
            border_color,
            background_color,
            has_close_title,
            size_constraint,
            |base| {
                controller_builder(YesNoControllerInner {
                    base,
                    is_showing_buttons: true,
                })
            },
            |controller| {
                VerticalSplit::new(
                    content_builder(controller),
                    Padded::new(
                        Padding::vertical(1),
                        ButtonsOrTextLine {
                            controller: Rc::clone(controller),
                            buttons: DualButtons::new(
                                redraw_notify,
                                yes_text,
                                no_text,
                                YES_KEYS,
                                CANCEL_NO_KEYS,
                                handler_builder(controller),
                                yes_style,
                                yes_selected_style,
                                no_style,
                                no_selected_style,
                            ),
                            alternative_text: CenteredTextLine::new(alternative_text, alternative_text_style),
                        },
                    ),
                    0,
                    0,
                )
            },
        );

        let value = YesNoPopup { base };
        (value, receiver)
    }
}

impl<C: YesNoPopupController, T: PopupContent, H: DualButtonsHandler> UIElement for YesNoPopup<C, T, H> {
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

pub struct YesNoClosureHandler<C: YesNoPopupController, YF: FnMut(Rc<C>), NF: FnMut(Rc<C>)> {
    controller: Weak<C>,
    on_yes: YF,
    on_no: NF,
}

impl<C: YesNoPopupController, YF: FnMut(Rc<C>), NF: FnMut(Rc<C>)> YesNoClosureHandler<C, YF, NF> {
    pub fn new(controller: &Rc<C>, on_yes: YF, on_no: NF) -> Self {
        Self {
            controller: Rc::downgrade(controller),
            on_yes,
            on_no,
        }
    }
}

impl<C: YesNoPopupController, YF: FnMut(Rc<C>), NF: FnMut(Rc<C>)> DualButtonsHandler for YesNoClosureHandler<C, YF, NF> {
    fn on_left(&mut self) {
        if let Some(rc) = self.controller.upgrade() {
            (self.on_yes)(rc);
        }
    }

    fn on_right(&mut self) {
        if let Some(rc) = self.controller.upgrade() {
            (self.on_no)(rc);
        }
    }
}
