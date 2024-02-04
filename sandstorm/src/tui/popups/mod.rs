use ratatui::{
    layout::Alignment,
    style::Color,
    widgets::block::{Position, Title},
};

pub mod confirm_close_popup;

const BACKGROUND_COLOR: Color = Color::Blue;

const CLOSE_KEY: char = 'q';
const CLOSE_TITLE: &str = "[close (q)]─";

const YES_KEY: char = 'y';
const YES_TITLE: &str = "[YES (y)]";
const YES_KEYS: &[char] = &[YES_KEY];

const NO_KEY: char = 'n';
const CANCEL_KEY: char = 'c';
const CANCEL_TITLE: &str = "[CANCEL (c/n)]";
const CANCEL_NO_KEYS: &[char] = &[NO_KEY, CANCEL_KEY];

fn get_close_title() -> Title<'static> {
    Title::from(CLOSE_TITLE).alignment(Alignment::Right).position(Position::Bottom)
}
