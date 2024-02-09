use std::rc::Rc;

use crossterm::event::{self, KeyCode, KeyEventKind};
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

pub struct TextEntry {
    redraw_notify: Rc<Notify>,
    text: String,
    text_len_chars: usize,
    text_idle_style: Style,
    text_typing_style: Style,
    max_length: usize,
    cursor: Option<CursorPosition>,
    current_position: (u16, u16),
}

struct CursorPosition {
    index_bytes: usize,
    index_chars: usize,
}

impl TextEntry {
    pub fn new(redraw_notify: Rc<Notify>, text: String, text_idle_style: Style, text_typing_style: Style, max_length: usize) -> Self {
        Self {
            redraw_notify,
            text_len_chars: text.chars().count(),
            text,
            text_idle_style: text_idle_style.underlined(),
            text_typing_style: text_typing_style.underlined(),
            max_length,
            cursor: None,
            current_position: (0, 0),
        }
    }

    fn get_focus_position(&self) -> (u16, u16) {
        self.current_position
    }
}

impl UIElement for TextEntry {
    fn resize(&mut self, area: Rect) {
        self.current_position = (area.x, area.y);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let max_text_x = if let Some(cursor_position) = &self.cursor {
            let mut from_char_index: isize = cursor_position.index_chars as isize - (area.width as isize + 1) / 2;
            let mut to_char_index: usize = cursor_position.index_chars + area.width as usize / 2;

            if from_char_index < 0 {
                to_char_index += (-from_char_index) as usize;
                from_char_index = 0;
            }
            let mut from_char_index = from_char_index as usize;

            if to_char_index > self.text_len_chars {
                from_char_index = from_char_index.saturating_sub(to_char_index - self.text_len_chars);
                to_char_index = self.text_len_chars;
            }

            let mut max_chars_before = cursor_position.index_chars.saturating_sub(from_char_index) as u16;
            let max_chars_after = to_char_index.saturating_sub(cursor_position.index_chars) as u16;

            if cursor_position.index_chars != self.max_length && max_chars_before == area.width {
                max_chars_before = max_chars_before.saturating_sub(1);
            }

            let buf = frame.buffer_mut();

            let mut index = buf.index_of(area.x + max_chars_before, area.y);
            let chars_before = self.text[..cursor_position.index_bytes].chars().rev();
            for c in chars_before.take(max_chars_before as usize) {
                index -= 1;
                buf.content[index].set_char(c).set_style(self.text_typing_style);
            }

            let mut index = buf.index_of(area.x + max_chars_before, area.y);
            let chars_after = self.text[cursor_position.index_bytes..].chars();
            for c in chars_after.take(max_chars_after as usize) {
                buf.content[index].set_char(c).set_style(self.text_typing_style);
                index += 1;
            }

            frame.set_cursor(area.x + max_chars_before, area.y);

            area.x + max_chars_before + max_chars_after
        } else {
            frame
                .buffer_mut()
                .set_stringn(area.x, area.y, &self.text, area.width as usize, self.text_idle_style)
                .0
        };

        if max_text_x < area.right() {
            let text_style = match self.cursor.is_some() {
                true => self.text_typing_style,
                false => self.text_idle_style,
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
        let status = match key_event.code {
            KeyCode::Char(c) => {
                if self.text_len_chars < self.max_length {
                    self.text.insert(cursor_position.index_bytes, c);
                    self.text_len_chars += 1;
                    cursor_position.index_bytes += c.len_utf8();
                    cursor_position.index_chars += 1;
                    needs_notify = true;
                }

                HandleEventStatus::Handled
            }
            KeyCode::Backspace => {
                // TODO: Implement Ctrl-Backspace
                if let Some((index, _char)) = self.text[..cursor_position.index_bytes].char_indices().next_back() {
                    self.text.remove(index);
                    self.text_len_chars -= 1;
                    cursor_position.index_bytes = index;
                    cursor_position.index_chars -= 1;
                    needs_notify = true;
                }

                HandleEventStatus::Handled
            }
            KeyCode::Delete => {
                // TODO: Implement Ctrl-Delete
                if cursor_position.index_bytes != self.text.len() {
                    self.text.remove(cursor_position.index_bytes);
                    self.text_len_chars -= 1;
                    needs_notify = true;
                }

                HandleEventStatus::Handled
            }
            KeyCode::Left => {
                // TODO: Implement Ctrl-Left
                if let Some((index, _char)) = self.text[..cursor_position.index_bytes].char_indices().next_back() {
                    cursor_position.index_bytes = index;
                    cursor_position.index_chars -= 1;
                    needs_notify = true;
                }

                HandleEventStatus::Handled
            }
            KeyCode::Right => {
                // TODO: Implement Ctrl-Right
                if let Some(c) = self.text[cursor_position.index_bytes..].chars().next() {
                    cursor_position.index_bytes += c.len_utf8();
                    cursor_position.index_chars += 1;
                    needs_notify = true;
                }

                HandleEventStatus::Handled
            }
            KeyCode::End => {
                if cursor_position.index_bytes != self.text.len() {
                    cursor_position.index_bytes = self.text.len();
                    cursor_position.index_chars = self.text_len_chars;
                    needs_notify = true;
                }

                HandleEventStatus::Handled
            }
            KeyCode::Home => {
                if cursor_position.index_bytes != 0 {
                    cursor_position.index_bytes = 0;
                    cursor_position.index_chars = 0;
                    needs_notify = true;
                }

                HandleEventStatus::Handled
            }
            KeyCode::Up => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
            KeyCode::Down => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
            KeyCode::Esc => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            _ => HandleEventStatus::Unhandled,
        };

        if needs_notify {
            self.redraw_notify.notify_one();
        }

        status
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        self.cursor = Some(CursorPosition {
            index_bytes: self.text.len(),
            index_chars: self.text_len_chars,
        });

        self.redraw_notify.notify_one();

        true
    }

    fn focus_lost(&mut self) {
        self.cursor = None;
        self.redraw_notify.notify_one();
    }
}

impl PopupContent for TextEntry {
    fn begin_resize(&mut self, width: u16, _height: u16) -> (u16, u16) {
        (self.max_length.min(width as usize) as u16, 1)
    }
}
