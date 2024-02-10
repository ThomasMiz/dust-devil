use std::{cell::RefCell, rc::Rc};

use crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    Frame,
};
use tokio::sync::Notify;

use crate::tui::{
    popups::PopupContent,
    ui_element::{HandleEventStatus, PassFocusDirection, UIElement},
};

// An interface for handling events from inside a text entry.
pub trait TextEntryHandler {
    /// Called when ENTER is pressed while typing on the text entry. Returns whether the key event
    /// should be marked as handled, unhandled, or pass focus away.
    fn on_enter(&mut self, controller: &Rc<TextEntryController>) -> OnEnterResult;

    // Called before a character is entered or removed from the text in the entry. Returns true if
    // the character should be kept, or false to discard it.
    fn on_char(&mut self, controller: &Rc<TextEntryController>, c: char, cursor: &CursorPosition) -> bool;

    // Called after a character is entered or removed from the text in the entry. Returns true if
    // the entry should be kept in focus, or false to request focus be taken away.
    fn on_text_changed(&mut self, controller: &Rc<TextEntryController>) -> bool;
}

pub enum OnEnterResult {
    Handled,
    Unhandled,
    PassFocusAway,
}

pub struct TextEntry<H: TextEntryHandler> {
    controller: Rc<TextEntryController>,
    max_length: usize,
    cursor: Option<CursorPosition>,
    pub handler: H,
    current_position: (u16, u16),
}

pub struct TextEntryController {
    redraw_notify: Rc<Notify>,
    inner: RefCell<TextEntryControllerInner>,
}

struct TextEntryControllerInner {
    text: String,
    text_len_chars: usize,
    text_idle_style: Style,
    text_typing_style: Style,
}

impl TextEntryController {
    pub fn with_text<T, F: FnOnce(&str) -> T>(&self, f: F) -> T {
        let inner = self.inner.borrow();
        f(&inner.text)
    }

    pub fn get_idle_style(&self) -> Style {
        self.inner.borrow().text_idle_style
    }

    pub fn get_typing_style(&self) -> Style {
        self.inner.borrow().text_typing_style
    }

    pub fn modify_idle_style<F: FnOnce(&mut Style)>(&self, f: F) {
        let mut inner = self.inner.borrow_mut();
        f(&mut inner.text_idle_style);
        self.redraw_notify.notify_one();
    }

    pub fn modify_typing_style<F: FnOnce(&mut Style)>(&self, f: F) {
        let mut inner = self.inner.borrow_mut();
        f(&mut inner.text_typing_style);
        self.redraw_notify.notify_one();
    }
}

pub struct CursorPosition {
    pub index_bytes: usize,
    pub index_chars: usize,
}

impl<H: TextEntryHandler> TextEntry<H> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        text: String,
        text_idle_style: Style,
        text_typing_style: Style,
        max_length: usize,
        handler: H,
    ) -> Self {
        let inner = TextEntryControllerInner {
            text_len_chars: text.chars().count(),
            text,
            text_idle_style: text_idle_style.underlined(),
            text_typing_style: text_typing_style.underlined(),
        };

        let controller = TextEntryController {
            redraw_notify,
            inner: RefCell::new(inner),
        };

        Self {
            controller: Rc::new(controller),
            max_length,
            cursor: None,
            current_position: (0, 0),
            handler,
        }
    }

    pub fn controller(&self) -> Rc<TextEntryController> {
        Rc::clone(&self.controller)
    }

    pub fn is_typing(&self) -> bool {
        self.cursor.is_some()
    }

    fn get_focus_position(&self) -> (u16, u16) {
        self.current_position
    }
}

/// Calculates how many characters to skip left when skipping a whole word (with Ctrl-Left or
/// Ctrl-Backspace). The cursor is assumed to be at the end of the given string slice.
///
/// Returns a tuple with (bytes_to_skip, chars_to_skip).
fn calc_wordskip_left(s: &str) -> (usize, usize) {
    let mut chars = s.chars().rev().peekable();
    let mut byte_count = 0;
    let mut char_count = 0;

    while chars.peek().is_some_and(|c| !c.is_alphanumeric()) {
        let c = chars.next().unwrap();
        byte_count += c.len_utf8();
        char_count += 1;
    }

    while let Some(c) = chars.next().filter(|c| c.is_alphanumeric()) {
        byte_count += c.len_utf8();
        char_count += 1;
    }

    (byte_count, char_count)
}

/// Calculates how many characters to skip right when skipping a whole word (with Ctrl-Right or
/// Ctrl-Delete). The cursor is assumed to be at the start of the given string slice.
///
/// Returns a tuple with (bytes_to_skip, chars_to_skip).
fn calc_wordskip_right(s: &str) -> (usize, usize) {
    let mut chars = s.chars().peekable();
    let mut byte_count = 0;
    let mut char_count = 0;

    while chars.peek().is_some_and(|c| c.is_alphanumeric()) {
        let c = chars.next().unwrap();
        byte_count += c.len_utf8();
        char_count += 1;
    }

    while let Some(c) = chars.next().filter(|c| !c.is_alphanumeric()) {
        byte_count += c.len_utf8();
        char_count += 1;
    }

    (byte_count, char_count)
}

impl<H: TextEntryHandler> UIElement for TextEntry<H> {
    fn resize(&mut self, area: Rect) {
        self.current_position = (area.x, area.y);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let controller_inner = self.controller.inner.borrow();

        let max_text_x = if let Some(cursor_position) = &self.cursor {
            let mut from_char_index: isize = cursor_position.index_chars as isize - (area.width as isize + 1) / 2;
            let mut to_char_index: usize = cursor_position.index_chars + area.width as usize / 2;

            if from_char_index < 0 {
                to_char_index += (-from_char_index) as usize;
                from_char_index = 0;
            }
            let mut from_char_index = from_char_index as usize;

            if to_char_index > controller_inner.text_len_chars {
                from_char_index = from_char_index.saturating_sub(to_char_index - controller_inner.text_len_chars);
                to_char_index = controller_inner.text_len_chars;
            }

            let mut max_chars_before = cursor_position.index_chars.saturating_sub(from_char_index) as u16;
            let max_chars_after = to_char_index.saturating_sub(cursor_position.index_chars) as u16;

            if cursor_position.index_chars != self.max_length && max_chars_before == area.width {
                max_chars_before = max_chars_before.saturating_sub(1);
            }

            let buf = frame.buffer_mut();

            let mut index = buf.index_of(area.x + max_chars_before, area.y);
            let chars_before = controller_inner.text[..cursor_position.index_bytes].chars().rev();
            for c in chars_before.take(max_chars_before as usize) {
                index -= 1;
                buf.content[index].set_char(c).set_style(controller_inner.text_typing_style);
            }

            let mut index = buf.index_of(area.x + max_chars_before, area.y);
            let chars_after = controller_inner.text[cursor_position.index_bytes..].chars();
            for c in chars_after.take(max_chars_after as usize) {
                buf.content[index].set_char(c).set_style(controller_inner.text_typing_style);
                index += 1;
            }

            frame.set_cursor(area.x + max_chars_before, area.y);

            area.x + max_chars_before + max_chars_after
        } else {
            frame
                .buffer_mut()
                .set_stringn(
                    area.x,
                    area.y,
                    &controller_inner.text,
                    area.width as usize,
                    controller_inner.text_idle_style,
                )
                .0
        };

        if max_text_x < area.right() {
            let text_style = match self.cursor.is_some() {
                true => controller_inner.text_typing_style,
                false => controller_inner.text_idle_style,
            };

            let buf = frame.buffer_mut();
            let index = buf.index_of(max_text_x, area.y);
            for i in 0..(area.right() - max_text_x) {
                buf.content[index + i as usize].set_style(text_style).set_char(' ');
            }
        }
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        if !is_focused {
            return HandleEventStatus::Unhandled;
        }

        let cursor_position = match self.cursor.as_mut() {
            Some(c) => c,
            None => return HandleEventStatus::Unhandled,
        };

        let key_event = match event {
            event::Event::Key(e) if e.kind != KeyEventKind::Release => e,
            _ => return HandleEventStatus::Unhandled,
        };

        let mut needs_notify = false;
        let mut text_changed = true;

        match key_event.code {
            KeyCode::Char(c) if self.controller.inner.borrow().text_len_chars < self.max_length => {
                if self.handler.on_char(&self.controller, c, cursor_position) {
                    let mut controller_inner = self.controller.inner.borrow_mut();
                    controller_inner.text.insert(cursor_position.index_bytes, c);
                    controller_inner.text_len_chars += 1;
                    cursor_position.index_bytes += c.len_utf8();
                    cursor_position.index_chars += 1;
                    text_changed = true;
                }
            }
            KeyCode::Backspace => {
                let mut controller_inner = self.controller.inner.borrow_mut();
                let (byte_count, char_count) = if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                    calc_wordskip_left(&controller_inner.text[..cursor_position.index_bytes])
                } else {
                    controller_inner.text[..cursor_position.index_bytes]
                        .chars()
                        .next_back()
                        .map(|c| (c.len_utf8(), 1))
                        .unwrap_or((0, 0))
                };

                if byte_count != 0 {
                    let from_byte = cursor_position.index_bytes - byte_count;
                    controller_inner.text.drain(from_byte..cursor_position.index_bytes);
                    controller_inner.text_len_chars -= char_count;
                    cursor_position.index_bytes -= byte_count;
                    cursor_position.index_chars -= char_count;
                    text_changed = true;
                }
            }
            KeyCode::Delete => {
                let mut controller_inner = self.controller.inner.borrow_mut();
                let (byte_count, char_count) = if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                    calc_wordskip_right(&controller_inner.text[cursor_position.index_bytes..])
                } else {
                    controller_inner.text[cursor_position.index_bytes..]
                        .chars()
                        .next()
                        .map(|c| (c.len_utf8(), 1))
                        .unwrap_or((0, 0))
                };

                if byte_count != 0 {
                    let to_byte = cursor_position.index_bytes + byte_count;
                    controller_inner.text.drain(cursor_position.index_bytes..to_byte);
                    controller_inner.text_len_chars -= char_count;
                    text_changed = true;
                }
            }
            KeyCode::Left => {
                let controller_inner = self.controller.inner.borrow();
                let (byte_count, char_count) = if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                    calc_wordskip_left(&controller_inner.text[..cursor_position.index_bytes])
                } else {
                    controller_inner.text[..cursor_position.index_bytes]
                        .chars()
                        .next_back()
                        .map(|c| (c.len_utf8(), 1))
                        .unwrap_or((0, 0))
                };

                if byte_count != 0 {
                    cursor_position.index_bytes -= byte_count;
                    cursor_position.index_chars -= char_count;
                    needs_notify = true;
                }
            }
            KeyCode::Right => {
                let controller_inner = self.controller.inner.borrow();
                let (byte_count, char_count) = if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                    calc_wordskip_right(&controller_inner.text[cursor_position.index_bytes..])
                } else {
                    controller_inner.text[cursor_position.index_bytes..]
                        .chars()
                        .next()
                        .map(|c| (c.len_utf8(), 1))
                        .unwrap_or((0, 0))
                };

                if byte_count != 0 {
                    cursor_position.index_bytes += byte_count;
                    cursor_position.index_chars += char_count;
                    needs_notify = true;
                }
            }
            KeyCode::End => {
                let controller_inner = self.controller.inner.borrow();
                if cursor_position.index_bytes != controller_inner.text.len() {
                    cursor_position.index_bytes = controller_inner.text.len();
                    cursor_position.index_chars = controller_inner.text_len_chars;
                    needs_notify = true;
                }
            }
            KeyCode::Home => {
                if cursor_position.index_bytes != 0 {
                    cursor_position.index_bytes = 0;
                    cursor_position.index_chars = 0;
                    needs_notify = true;
                }
            }
            KeyCode::Up => return HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
            KeyCode::Down => return HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
            KeyCode::Esc => return HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            KeyCode::Enter => {
                return match self.handler.on_enter(&self.controller) {
                    OnEnterResult::Handled => HandleEventStatus::Handled,
                    OnEnterResult::Unhandled => HandleEventStatus::Unhandled,
                    OnEnterResult::PassFocusAway => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
                }
            }
            _ => return HandleEventStatus::Unhandled,
        };

        if text_changed || needs_notify {
            self.controller.redraw_notify.notify_one();
        }

        match text_changed && !self.handler.on_text_changed(&self.controller) {
            true => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            false => HandleEventStatus::Handled,
        }
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        let controller_inner = self.controller.inner.borrow();
        self.cursor = Some(CursorPosition {
            index_bytes: controller_inner.text.len(),
            index_chars: controller_inner.text_len_chars,
        });

        self.controller.redraw_notify.notify_one();

        true
    }

    fn focus_lost(&mut self) {
        self.cursor = None;
        self.controller.redraw_notify.notify_one();
    }
}

impl<H: TextEntryHandler> PopupContent for TextEntry<H> {
    fn begin_resize(&mut self, width: u16, _height: u16) -> (u16, u16) {
        (self.max_length.min(width as usize) as u16, 1)
    }
}
