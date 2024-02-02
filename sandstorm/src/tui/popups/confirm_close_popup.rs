use std::rc::Rc;

use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget, Wrap},
};
use tokio::sync::{oneshot, Notify};

use crate::tui::ui_element::{HandleEventStatus, UIElement};

pub struct ConfirmClosePopup {
    redraw_notify: Rc<Notify>,
    screen_area: Rect,
    current_size: (u16, u16),
    close_sender: Option<oneshot::Sender<()>>,
    shutdown_notify: Rc<Notify>,
}

impl ConfirmClosePopup {
    pub fn new(redraw_notify: Rc<Notify>, shutdown_notify: Rc<Notify>) -> (Self, oneshot::Receiver<()>) {
        let (close_sender, close_receiver) = oneshot::channel();

        let value = Self {
            redraw_notify,
            screen_area: Rect::default(),
            current_size: (0, 0),
            close_sender: Some(close_sender),
            shutdown_notify,
        };

        (value, close_receiver)
    }
}

impl UIElement for ConfirmClosePopup {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.screen_area = area;
        self.current_size.0 = area.width.min(45).saturating_sub(5);
        self.current_size.1 = area.height.min(25).saturating_sub(5);

        let popup_area = Rect::new(5, 5, self.current_size.0, self.current_size.1);
        Clear.render(popup_area, buf);

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::reset().fg(Color::Red))
            .title_alignment(Alignment::Left)
            .title("─Close");

        let text_area = block.inner(popup_area);
        block.render(popup_area, buf);

        Paragraph::new("A ver flaco si te muestro un cartel así me das bola lcdll")
            .wrap(Wrap { trim: true })
            .render(text_area, buf);
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        if !is_focused {
            return HandleEventStatus::Unhandled;
        }

        let key_event = match event {
            event::Event::Key(key_event) if key_event.kind != KeyEventKind::Release => key_event,
            _ => return HandleEventStatus::Unhandled,
        };

        if key_event.code == KeyCode::Esc {
            if let Some(close_sender) = self.close_sender.take() {
                let _ = close_sender.send(());
                return HandleEventStatus::Handled;
            }
        }

        HandleEventStatus::Unhandled
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        true
    }

    fn focus_lost(&mut self) {}
}
