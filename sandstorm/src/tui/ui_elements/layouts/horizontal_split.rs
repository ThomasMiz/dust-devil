use std::{
    io::{Error, Write},
    num::NonZeroU16,
};

use crate::tui::{
    types::{HorizontalLine, Point, Rectangle},
    ui_elements::UIElement,
};

pub struct HorizontalSplit<L: UIElement, R: UIElement> {
    area: Rectangle,
    left_width: u16,
    left: L,
    right: R,
}

#[inline]
fn calculate_left_area(area: &Rectangle, left_width: u16) -> Option<Rectangle> {
    NonZeroU16::new(left_width).map(|width| Rectangle::new(area.top_left, area.width.min(width), area.height))
}

#[inline]
fn calculate_right_area(area: &Rectangle, left_width: u16) -> Option<Rectangle> {
    let top_left = Point::new(area.top_left.x + left_width, area.top_left.y);

    // Note: checking that `top_left.x < area.right()` is enough to ensure there is at
    // least one pixel of width for the right area. Proof:
    // top_left.x <= area.right() }--> Expand both sides
    // area.top_left.x + left_width <= area.top_left.x + area.width() - 1 }--> Cancel out area.top_left.x
    // left_width <= area.width() - 1
    // left_width < area.width()
    // 0 < area.width() - left_width

    if top_left.x <= area.right() {
        // SAFETY: We ensured that 0 < area.width() - left_width.
        let width = unsafe { NonZeroU16::new_unchecked(area.width() - left_width) };
        Some(Rectangle::new(top_left, width, area.height))
    } else {
        None
    }
}

impl<L: UIElement, R: UIElement> HorizontalSplit<L, R> {
    pub fn new<LB, RB>(area: Rectangle, left_width: u16, left_builder: LB, right_builder: RB) -> Self
    where
        LB: FnOnce(Rectangle) -> L,
        RB: FnOnce(Rectangle) -> R,
    {
        let left_area = calculate_left_area(&area, left_width).unwrap_or(Rectangle::get_single_pixel_rect());
        let right_area = calculate_right_area(&area, left_width).unwrap_or(Rectangle::get_single_pixel_rect());

        Self {
            area,
            left_width,
            left: left_builder(left_area),
            right: right_builder(right_area),
        }
    }

    pub fn left_area(&self) -> Option<Rectangle> {
        calculate_left_area(&self.area, self.left_width)
    }

    pub fn right_area(&self) -> Option<Rectangle> {
        calculate_right_area(&self.area, self.left_width)
    }

    pub fn left(&self) -> &L {
        &self.left
    }

    pub fn right(&self) -> &R {
        &self.right
    }

    pub fn left_mut(&mut self) -> &mut L {
        &mut self.left
    }

    pub fn right_mut(&mut self) -> &mut R {
        &mut self.right
    }
}

impl<L: UIElement, R: UIElement> UIElement for HorizontalSplit<L, R> {
    fn area(&self) -> Rectangle {
        self.area
    }

    fn draw_line<O: Write>(
        &mut self,
        out: &mut O,
        area: HorizontalLine,
        mut is_cursor_at_start: bool,
        force_redraw: bool,
    ) -> Result<bool, Error> {
        if let Some(Some(left_area)) = self.left_area().map(|rect| area.intersection_with_rect(rect)) {
            is_cursor_at_start = self.left.draw_line(out, left_area, is_cursor_at_start, force_redraw)?;
        }

        if let Some(Some(right_area)) = self.right_area().map(|rect| area.intersection_with_rect(rect)) {
            is_cursor_at_start = self.right.draw_line(out, right_area, is_cursor_at_start, force_redraw)?;
        }

        Ok(is_cursor_at_start)
    }

    fn resize(&mut self, area: Rectangle) {
        self.area = area;

        if let Some(left_area) = self.left_area() {
            self.left.resize(left_area);
        }

        if let Some(right_area) = self.right_area() {
            self.right.resize(right_area);
        }
    }
}
