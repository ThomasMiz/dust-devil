use std::rc::Rc;

use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{
        block::{Position, Title},
        Block, BorderType, Borders, Clear, Padding, Widget,
    },
};
use tokio::sync::{oneshot, Notify};

use crate::tui::{
    elements::{
        centered_text_line::CenteredTextLine,
        dual_buttons::{Button, DualButtons, DualButtonsEventStatus},
    },
    text_wrapper::WrapTextIter,
    ui_element::{HandleEventStatus, PassFocusDirection, UIElement},
};

use super::{get_close_title, BACKGROUND_COLOR, CANCEL_NO_KEYS, CANCEL_TITLE, CLOSE_KEY, YES_KEYS, YES_TITLE};

const TITLE: &str = "─Close";
const PROMPT: &str = "Are you sure you want to close this terminal?";
const CLOSING_MESSAGE: &str = "Closing...";
const POPUP_WIDTH: u16 = 34;
const TEXT_STYLE: Style = Style::new();

pub struct ConfirmClosePopup {
    redraw_notify: Rc<Notify>,
    screen_area: Rect,
    current_size: (u16, u16),
    close_sender: Option<oneshot::Sender<()>>,
    shutdown_notify: Rc<Notify>,
    prompt_lines: Vec<CenteredTextLine<'static>>,
    focused_element: FocusedElement,
    popup_state: PopupState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedElement {
    None,
    DualButtons,
}

enum PopupState {
    Buttons(DualButtons<'static>),
    ClosingDown(CenteredTextLine<'static>),
}

impl ConfirmClosePopup {
    pub fn new(redraw_notify: Rc<Notify>, shutdown_notify: Rc<Notify>) -> (Self, oneshot::Receiver<()>) {
        let (close_sender, close_receiver) = oneshot::channel();

        let prompt_lines: Vec<_> = WrapTextIter::new(PROMPT, POPUP_WIDTH as usize)
            .map(|line| CenteredTextLine::new(line, TEXT_STYLE))
            .collect();

        let dual_buttons = DualButtons::new(
            redraw_notify.clone(),
            YES_TITLE,
            CANCEL_TITLE,
            YES_KEYS,
            CANCEL_NO_KEYS,
            Style::new(),
            Style::new().bg(Color::LightBlue),
            Style::new(),
            Style::new().bg(Color::LightBlue),
        );

        let value = Self {
            redraw_notify,
            screen_area: Rect::default(),
            current_size: (0, 0),
            close_sender: Some(close_sender),
            shutdown_notify,
            prompt_lines,
            focused_element: FocusedElement::None,
            popup_state: PopupState::Buttons(dual_buttons),
        };

        (value, close_receiver)
    }

    fn on_yes_selected(&mut self) {
        if let PopupState::ClosingDown(_) = self.popup_state {
            return;
        }

        self.shutdown_notify.notify_one();
        self.popup_state = PopupState::ClosingDown(CenteredTextLine::new(CLOSING_MESSAGE, Style::reset()));
        self.redraw_notify.notify_one();
    }

    fn close(&mut self) {
        if let Some(close_sender) = self.close_sender.take() {
            let _ = close_sender.send(());
        }
    }
}

impl UIElement for ConfirmClosePopup {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.screen_area = area;
        self.current_size.0 = area.width.min(POPUP_WIDTH);
        self.current_size.1 = area.height.min(self.prompt_lines.len() as u16 + 5);

        let popup_area = Rect::new(
            (area.width - self.current_size.0) / 2,
            (area.height - self.current_size.1) / 2,
            self.current_size.0,
            self.current_size.1,
        );

        Clear.render(popup_area, buf);

        let title = Title::from(TITLE).alignment(Alignment::Left).position(Position::Top);

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::reset().bg(BACKGROUND_COLOR))
            .padding(Padding::horizontal(1))
            .title(title)
            .title(get_close_title());

        let inner_area = block.inner(popup_area);
        block.render(popup_area, buf);

        for i in 0..(inner_area.height.min(self.prompt_lines.len() as u16)) {
            let mut prompt_area = inner_area;
            prompt_area.height = 1;
            prompt_area.y += i;
            self.prompt_lines[i as usize].render(prompt_area, buf);
        }

        let buttons_y = inner_area.y + self.prompt_lines.len() as u16 + 1;
        if buttons_y < inner_area.bottom() {
            let mut buttons_area = inner_area;
            buttons_area.y = buttons_y;
            buttons_area.height = 1;

            match &mut self.popup_state {
                PopupState::Buttons(dual_buttons) => dual_buttons.render(buttons_area, buf),
                PopupState::ClosingDown(closing_text) => closing_text.render(buttons_area, buf),
            }
        }
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        let dual_buttons = match &mut self.popup_state {
            PopupState::Buttons(b) => b,
            PopupState::ClosingDown(_) => return HandleEventStatus::Handled,
        };

        if !is_focused {
            return HandleEventStatus::Unhandled;
        }

        if self.focused_element == FocusedElement::None {
            let key_event = match event {
                event::Event::Key(e) if e.kind != KeyEventKind::Release => e,
                _ => return HandleEventStatus::Unhandled,
            };

            let unfocused_element_result = match key_event.code {
                KeyCode::Char(CLOSE_KEY) => {
                    self.close();
                    HandleEventStatus::Handled
                }
                KeyCode::Char(_) => match dual_buttons.handle_event_with_result(event, false) {
                    DualButtonsEventStatus::Handled(Button::None) => HandleEventStatus::Handled,
                    DualButtonsEventStatus::Handled(Button::Left) => {
                        self.on_yes_selected();
                        HandleEventStatus::Handled
                    }
                    DualButtonsEventStatus::Handled(Button::Right) => {
                        self.close();
                        HandleEventStatus::Handled
                    }
                    _ => HandleEventStatus::Unhandled,
                },
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                    let focus_position_x = match key_event.code {
                        KeyCode::Left => self.screen_area.right(),
                        _ => self.screen_area.left(),
                    };

                    match dual_buttons.receive_focus((focus_position_x, 0)) {
                        true => {
                            self.focused_element = FocusedElement::DualButtons;
                            HandleEventStatus::Handled
                        }
                        false => HandleEventStatus::Unhandled,
                    }
                }
                KeyCode::Esc => {
                    self.close();
                    HandleEventStatus::Handled
                }
                _ => HandleEventStatus::Unhandled,
            };

            return unfocused_element_result;
        }

        match dual_buttons.handle_event_with_result(event, true) {
            DualButtonsEventStatus::Handled(Button::None) => HandleEventStatus::Handled,
            DualButtonsEventStatus::Handled(Button::Left) => {
                self.on_yes_selected();
                HandleEventStatus::Handled
            }
            DualButtonsEventStatus::Handled(Button::Right) => {
                self.close();
                HandleEventStatus::Handled
            }
            DualButtonsEventStatus::PassFocus(_, PassFocusDirection::Away) => {
                dual_buttons.focus_lost();
                self.focused_element = FocusedElement::None;
                HandleEventStatus::Handled
            }
            DualButtonsEventStatus::Unhandled => {
                if let event::Event::Key(key_event) = event {
                    match key_event.code {
                        _ if key_event.kind == KeyEventKind::Release => HandleEventStatus::Unhandled,
                        KeyCode::Esc => {
                            self.close();
                            HandleEventStatus::Handled
                        }
                        KeyCode::Char(c) if c.to_ascii_lowercase() == CLOSE_KEY => {
                            self.close();
                            HandleEventStatus::Handled
                        }
                        _ => HandleEventStatus::Unhandled,
                    }
                } else {
                    HandleEventStatus::Unhandled
                }
            }
            _ => HandleEventStatus::Unhandled,
        }
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        true
    }

    fn focus_lost(&mut self) {}
}
