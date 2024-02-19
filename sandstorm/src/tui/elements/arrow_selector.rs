use std::{ops::Deref, rc::Rc};

use crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{layout::Rect, style::Style, Frame};
use tokio::sync::Notify;

use crate::tui::{
    text_wrapper::StaticString,
    ui_element::{AutosizeUIElement, HandleEventStatus, PassFocusDirection, UIElement},
};

pub trait ArrowSelectorHandler {
    fn selection_changed(&mut self, selected_index: usize);
}

pub struct ArrowSelector<H: ArrowSelectorHandler> {
    redraw_notify: Rc<Notify>,
    options: Vec<(StaticString, Option<char>)>,
    selected_index: usize,
    options_idle_style: Style,
    options_focused_style: Style,
    options_selecting_style: Style,
    arrows_selecting_style: Style,
    options_max_width: u16,
    after_text: StaticString,
    after_text_len: u16,
    autoselect: bool,
    pub handler: H,
    is_focused: bool,
    is_selecting: bool,
    current_position: (u16, u16),
}

impl<H: ArrowSelectorHandler> ArrowSelector<H> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        redraw_notify: Rc<Notify>,
        mut options: Vec<(StaticString, Option<char>)>,
        selected_index: usize,
        options_idle_style: Style,
        options_focused_style: Style,
        options_selecting_style: Style,
        arrows_selecting_style: Style,
        after_text: StaticString,
        autoselect: bool,
        handler: H,
    ) -> Self {
        if options.is_empty() {
            options.push((" ".into(), None));
        }

        let options_max_width = options.iter().map(|(s, _)| s.deref().chars().count()).max();
        let options_max_width = options_max_width.unwrap_or(0).min(u16::MAX as usize) as u16;
        let selected_index = selected_index.min(options.len());

        for (_, ch) in options.iter_mut() {
            *ch = ch.map(|c| c.to_ascii_lowercase());
        }

        let after_text_len = after_text.chars().count().min(u16::MAX as usize) as u16;

        Self {
            redraw_notify,
            options,
            selected_index,
            options_idle_style,
            options_focused_style,
            options_selecting_style,
            arrows_selecting_style,
            options_max_width,
            after_text,
            after_text_len,
            autoselect,
            handler,
            is_focused: false,
            is_selecting: false,
            current_position: (0, 0),
        }
    }

    pub fn set_selected_index_no_redraw(&mut self, selected_index: usize) {
        if selected_index >= self.options.len() {
            panic!("Attempted to set selected index of arrow selector with selected_index >= options.len()");
        }

        self.selected_index = selected_index;
    }

    fn get_focus_position(&self) -> (u16, u16) {
        self.current_position
    }
}

impl<H: ArrowSelectorHandler> UIElement for ArrowSelector<H> {
    fn resize(&mut self, area: Rect) {
        self.current_position = (area.x, area.y);
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let buf = frame.buffer_mut();

        let (options_style, arrows_style) = match (self.is_focused, self.is_selecting) {
            (true, true) => (self.options_selecting_style, Some(self.arrows_selecting_style)),
            (true, false) => (self.options_focused_style, None),
            (false, _) => (self.options_idle_style, None),
        };

        let mut draw_x = area.x;

        if let Some(style) = arrows_style {
            let (x, _) = buf.set_stringn(draw_x, area.y, "← ", (area.right() - draw_x) as usize, style);
            draw_x = x;
        }

        if draw_x >= area.right() {
            return;
        }

        let option_str = self.options[self.selected_index].0.deref();
        let (x, _) = buf.set_stringn(draw_x, area.y, option_str, (area.right() - draw_x) as usize, options_style);
        draw_x = x;

        if draw_x >= area.right() {
            return;
        }

        if let Some(style) = arrows_style {
            let (x, _) = buf.set_stringn(draw_x, area.y, " →", (area.right() - draw_x) as usize, style);
            draw_x = x;
        }

        draw_x += 1;
        if draw_x >= area.right() {
            return;
        }

        let after_str = self.after_text.deref();
        buf.set_stringn(draw_x, area.y, after_str, (area.right() - draw_x) as usize, self.options_idle_style);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        let key_event = match event {
            event::Event::Key(e) if e.kind != KeyEventKind::Release => e,
            _ => return HandleEventStatus::Unhandled,
        };

        if let KeyCode::Char(c) = key_event.code {
            let c = c.to_ascii_lowercase();
            let mut iter = self.options.iter().map(|(_, ch)| *ch).enumerate();
            let index = iter.find_map(|(idx, ch)| ch.filter(|ch| *ch == c).map(|_| idx));

            if let Some(new_selected_index) = index.filter(|idx| *idx != self.selected_index) {
                self.selected_index = new_selected_index;
                self.handler.selection_changed(new_selected_index);
                self.redraw_notify.notify_one();
                return HandleEventStatus::Handled;
            }

            return HandleEventStatus::Unhandled;
        }

        if !is_focused {
            return HandleEventStatus::Unhandled;
        }

        if self.is_selecting
            && !key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Left | KeyCode::Right | KeyCode::Esc)
        {
            match key_event.code {
                KeyCode::Left => {
                    if self.selected_index != 0 {
                        self.selected_index -= 1;
                        self.handler.selection_changed(self.selected_index);
                        self.redraw_notify.notify_one();
                    }

                    return HandleEventStatus::Handled;
                }
                KeyCode::Right => {
                    if self.selected_index < self.options.len() - 1 {
                        self.selected_index += 1;
                        self.handler.selection_changed(self.selected_index);
                        self.redraw_notify.notify_one();
                    }

                    return HandleEventStatus::Handled;
                }
                KeyCode::Esc => {
                    self.is_selecting = false;
                    self.redraw_notify.notify_one();
                    return HandleEventStatus::Handled;
                }
                _ => {}
            };
        }

        match key_event.code {
            KeyCode::Enter => {
                self.is_selecting = !self.is_selecting;
                self.redraw_notify.notify_one();
                HandleEventStatus::Handled
            }
            KeyCode::Left => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Left),
            KeyCode::Right => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Right),
            KeyCode::Up => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
            KeyCode::Down => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
            KeyCode::Tab => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Forward),
            KeyCode::Esc => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            _ => HandleEventStatus::Unhandled,
        }
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        self.is_focused = true;
        self.is_selecting = self.autoselect;
        self.redraw_notify.notify_one();
        true
    }

    fn focus_lost(&mut self) {
        self.is_selecting = false;
        self.is_focused = false;
        self.redraw_notify.notify_one();
    }
}

impl<H: ArrowSelectorHandler> AutosizeUIElement for ArrowSelector<H> {
    fn begin_resize(&mut self, width: u16, _height: u16) -> (u16, u16) {
        let desired_width = self.options_max_width.saturating_add(4).saturating_add(self.after_text_len);
        (width.min(desired_width), 1)
    }
}
