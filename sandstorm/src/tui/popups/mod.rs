use ratatui::{
    layout::Alignment,
    style::{Color, Style},
    widgets::{
        block::{Position, Title},
        Block, BorderType, Borders, Padding,
    },
};

pub mod confirm_close_popup;
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

const POPUP_BLOCK_BASE: Block = Block::new()
    .borders(Borders::ALL)
    .border_type(BorderType::Plain)
    .padding(Padding::horizontal(1));

fn get_close_title() -> Title<'static> {
    Title::from(CLOSE_TITLE).alignment(Alignment::Right).position(Position::Bottom)
}

fn get_popup_block(title: &str, background_color: Color, close_title: bool) -> Block {
    let mut block = POPUP_BLOCK_BASE
        .style(Style::reset().bg(background_color))
        .title(Title::from(title).alignment(Alignment::Left).position(Position::Top));

    if close_title {
        block = block.title(get_close_title());
    }

    block
}
