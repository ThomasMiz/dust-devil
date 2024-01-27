use std::{
    io::{Error, Write},
    num::NonZeroU16,
};

use crate::tui::{
    types::{HorizontalLine, Point, Rectangle},
    ui_elements::UIElement,
};

pub struct VerticalSplit<U: UIElement, L: UIElement> {
    area: Rectangle,
    upper_height: u16,
    upper: U,
    lower: L,
}

#[inline]
fn calculate_upper_area(area: &Rectangle, upper_height: u16) -> Option<Rectangle> {
    NonZeroU16::new(upper_height).map(|height| Rectangle::new(area.top_left, area.width, area.height.min(height)))
}

#[inline]
fn calculate_lower_area(area: &Rectangle, upper_height: u16) -> Option<Rectangle> {
    let top_left = Point::new(area.top_left.x, area.top_left.y + upper_height);

    // Note: checking that `top_left.y < area.bottom()` is enough to ensure there is at
    // least one pixel of height for the lower area. Proof:
    // top_left.y < area.bottom() }--> Expand both sides
    // area.top_left.y + upper_height <= area.top_left.y + area.height() - 1 }--> Cancel out area.top_left.y
    // upper_height <= area.height() - 1
    // upper_height < area.height()
    // 0 < area.height() - upper_height

    if top_left.y <= area.bottom() {
        // SAFETY: We ensured that 0 < area.height() - upper_height.
        let height = unsafe { NonZeroU16::new_unchecked(area.height() - upper_height) };
        Some(Rectangle::new(top_left, area.width, height))
    } else {
        None
    }
}

impl<U: UIElement, L: UIElement> VerticalSplit<U, L> {
    pub fn new<UB, LB>(area: Rectangle, upper_height: u16, upper_builder: UB, lower_builder: LB) -> Self
    where
        UB: FnOnce(Rectangle) -> U,
        LB: FnOnce(Rectangle) -> L,
    {
        let upper_area = calculate_upper_area(&area, upper_height).unwrap_or(Rectangle::get_single_pixel_rect());
        let lower_area = calculate_lower_area(&area, upper_height).unwrap_or(Rectangle::get_single_pixel_rect());

        Self {
            area,
            upper_height,
            upper: upper_builder(upper_area),
            lower: lower_builder(lower_area),
        }
    }

    pub fn upper_area(&self) -> Option<Rectangle> {
        calculate_upper_area(&self.area, self.upper_height)
    }

    pub fn lower_area(&self) -> Option<Rectangle> {
        calculate_lower_area(&self.area, self.upper_height)
    }
}

impl<U: UIElement, L: UIElement> UIElement for VerticalSplit<U, L> {
    fn area(&self) -> Rectangle {
        self.area
    }

    fn draw_line<O: Write>(
        &mut self,
        out: &mut O,
        area: HorizontalLine,
        is_cursor_at_start: bool,
        force_redraw: bool,
    ) -> Result<bool, Error> {
        let lower_start_y = self.area.top() + self.upper_height;
        if area.y < lower_start_y {
            self.upper.draw_line(out, area, is_cursor_at_start, force_redraw)
        } else {
            self.lower.draw_line(out, area, is_cursor_at_start, force_redraw)
        }
    }

    fn resize(&mut self, area: Rectangle) {
        self.area = area;

        if let Some(upper_area) = self.upper_area() {
            self.upper.resize(upper_area);
        }

        if let Some(lower_area) = self.lower_area() {
            self.lower.resize(lower_area);
        }
    }
}
