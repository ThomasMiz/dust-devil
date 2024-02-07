use crossterm::event;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{
        block::{Position, Title},
        Block, BorderType, Borders,
    },
};

use super::ui_element::{HandleEventStatus, UIElement};

pub mod confirm_close_popup;
pub mod popup_base;
pub mod prompt_popup;
pub mod shutdown_popup;

const CLOSE_KEY: char = 'q';
const CLOSE_TITLE: &str = "[close (q)]─";

const YES_KEY: char = 'y';
const YES_TITLE: &str = "[YES (y)]";
const YES_KEYS: &[char] = &[YES_KEY];

const NO_KEY: char = 'n';
const CANCEL_KEY: char = 'c';
const CANCEL_TITLE: &str = "[CANCEL (c/n)]";
const CANCEL_NO_KEYS: &[char] = &[NO_KEY, CANCEL_KEY];

const POPUP_BLOCK_BASE: Block = Block::new().borders(Borders::ALL).border_type(BorderType::Plain);

fn get_close_title() -> Title<'static> {
    Title::from(CLOSE_TITLE).alignment(Alignment::Right).position(Position::Bottom)
}

fn get_popup_block(title: &str, background_color: Color, border_color: Color, close_title: bool) -> Block {
    let mut block = POPUP_BLOCK_BASE
        .style(Style::new().bg(background_color).fg(border_color))
        .title(Title::from(title).alignment(Alignment::Left).position(Position::Top));

    if close_title {
        block = block.title(get_close_title());
    }

    block
}

/// Represents an [`UIElement`] for a popup.
pub trait PopupContent: UIElement {
    /// Called before [`UIElement::resize`] with the maximum available size, and returns this
    /// element's desired size. After this call, `resize` will be called with the final size.
    ///
    /// Elements should only ask for exactly as much space as they need and no more. Asking for
    /// more space might mean other elements are not given any space at all.
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16);
}

/// An implementation of [`PopupContent`] that wraps around any [`UIElement`] and requests
/// a predetermined size.
pub struct SizedPopupContent<T: UIElement> {
    pub desired_size: (u16, u16),
    pub inner: T,
}

impl<T: UIElement> SizedPopupContent<T> {
    pub fn new(desired_size: (u16, u16), inner: T) -> Self {
        Self { desired_size, inner }
    }
}

impl<T: UIElement> UIElement for SizedPopupContent<T> {
    fn resize(&mut self, area: Rect) {
        self.inner.resize(area);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.inner.render(area, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.inner.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.inner.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.inner.focus_lost();
    }
}

impl<T: UIElement> PopupContent for SizedPopupContent<T> {
    fn begin_resize(&mut self, _width: u16, _height: u16) -> (u16, u16) {
        self.desired_size
    }
}

pub struct ConstrainedPopupContent<T: PopupContent> {
    pub size_constraint: (u16, u16),
    pub inner: T,
}

impl<T: PopupContent> ConstrainedPopupContent<T> {
    pub fn new(size_constraint: (u16, u16), inner: T) -> Self {
        Self { size_constraint, inner }
    }
}

impl<T: PopupContent> UIElement for ConstrainedPopupContent<T> {
    fn resize(&mut self, area: Rect) {
        self.inner.resize(area);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.inner.render(area, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        self.inner.handle_event(event, is_focused)
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.inner.receive_focus(focus_position)
    }

    fn focus_lost(&mut self) {
        self.inner.focus_lost();
    }
}

impl<T: PopupContent> PopupContent for ConstrainedPopupContent<T> {
    fn begin_resize(&mut self, mut width: u16, mut height: u16) -> (u16, u16) {
        width = width.min(self.size_constraint.0);
        height = height.min(self.size_constraint.1);
        self.inner.begin_resize(width, height)
    }
}
