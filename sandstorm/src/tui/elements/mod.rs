use super::ui_element::PassFocusDirection;

pub mod arrow_selector;
pub mod centered_button;
pub mod dual_buttons;
pub mod empty;
pub mod focus_cell;
pub mod horizontal_split;
pub mod long_list;
pub mod padded;
pub mod text;
pub mod text_entry;
pub mod vertical_split;

pub enum OnEnterResult {
    Handled,
    Unhandled,
    PassFocus(PassFocusDirection),
}
