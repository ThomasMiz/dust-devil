use ratatui::{
    style::{Color, Style},
    text::Line,
    widgets::{Block, BorderType, Borders},
};

pub mod auth_methods_popup;
pub mod buffer_size_popup;
pub mod confirm_close_popup;
pub mod loading_popup;
pub mod message_popup;
pub mod popup_base;
pub mod prompt_popup;
pub mod shutdown_popup;
pub mod size_constraint;
pub mod yes_no_popup;

pub const CLOSE_KEY: char = 'q';
pub const CLOSE_TITLE: &str = "[close (q)]─";

pub const YES_KEY: char = 'y';
pub const YES_TITLE: &str = "[YES (y)]";
pub const CONFIRM_TITLE: &str = "[CONFIRM (y)]";
pub const YES_KEYS: &[char] = &[YES_KEY];

pub const NO_KEY: char = 'n';
pub const CANCEL_KEY: char = 'c';
pub const CANCEL_TITLE: &str = "[CANCEL (c/n)]";
pub const CANCEL_NO_KEYS: &[char] = &[NO_KEY, CANCEL_KEY];

const POPUP_BLOCK_BASE: Block = Block::new().borders(Borders::ALL).border_type(BorderType::Plain);

fn get_popup_block(title: &str, background_color: Color, border_color: Color, close_title: bool) -> Block {
    let mut block = POPUP_BLOCK_BASE
        .style(Style::new().bg(background_color).fg(border_color))
        .title_top(Line::raw(title).left_aligned());

    if close_title {
        block = block.title_bottom(Line::raw(CLOSE_TITLE).right_aligned())
    }

    block
}
