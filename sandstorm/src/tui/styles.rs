#![allow(dead_code)]

use crossterm::style::{Attributes, Color};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameType {
    pub vertical: &'static str,
    pub horizontal: &'static str,
    pub top_left_corner: &'static str,
    pub top_right_corner: &'static str,
    pub bottom_left_corner: &'static str,
    pub bottom_right_corner: &'static str,
}

impl FrameType {
    const fn new(
        vertical: &'static str,
        horizontal: &'static str,
        top_left_corner: &'static str,
        top_right_corner: &'static str,
        bottom_left_corner: &'static str,
        bottom_right_corner: &'static str,
    ) -> Self {
        Self {
            vertical,
            horizontal,
            top_left_corner,
            top_right_corner,
            bottom_left_corner,
            bottom_right_corner,
        }
    }
}

pub mod frame_types {
    use crate::tui::chars;

    use super::FrameType;

    pub const LINE: FrameType = FrameType::new(
        chars::VERTICAL,
        chars::HORIZONTAL,
        chars::TOP_LEFT_CORNER,
        chars::TOP_RIGHT_CORNER,
        chars::BOTTOM_LEFT_CORNER,
        chars::BOTTOM_RIGHT_CORNER,
    );

    pub const CURVED: FrameType = FrameType::new(
        chars::VERTICAL,
        chars::HORIZONTAL,
        chars::TOP_LEFT_CORNER_CURVED,
        chars::TOP_RIGHT_CORNER_CURVED,
        chars::BOTTOM_LEFT_CORNER_CURVED,
        chars::BOTTOM_RIGHT_CORNER_CURVED,
    );

    pub const DOUBLE: FrameType = FrameType::new(
        chars::DOUBLE_VERTICAL,
        chars::DOUBLE_HORIZONTAL,
        chars::DOUBLE_TOP_LEFT_CORNER,
        chars::DOUBLE_TOP_RIGHT_CORNER,
        chars::DOUBLE_BOTTOM_LEFT_CORNER,
        chars::DOUBLE_BOTTOM_RIGHT_CORNER,
    );

    pub const THICK: FrameType = FrameType::new(
        chars::THICK_VERTICAL,
        chars::THICK_HORIZONTAL,
        chars::THICK_TOP_LEFT_CORNER,
        chars::THICK_TOP_RIGHT_CORNER,
        chars::THICK_BOTTOM_LEFT_CORNER,
        chars::THICK_BOTTOM_RIGHT_CORNER,
    );
}

pub struct ColorStyle {
    pub foreground: Color,
    pub background: Color,
}

impl ColorStyle {
    pub fn new(foreground: Color, background: Color) -> Self {
        Self { foreground, background }
    }
}

pub struct TextStyle {
    pub color: ColorStyle,
    pub attributes: Attributes,
}

impl TextStyle {
    pub fn new(color: ColorStyle, attributes: Attributes) -> Self {
        Self { color, attributes }
    }
}
