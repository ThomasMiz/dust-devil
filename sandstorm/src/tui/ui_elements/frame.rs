use std::io::{Error, Write};

use crossterm::{
    style::{ContentStyle, Print, PrintStyledContent, SetStyle, StyledContent},
    QueueableCommand,
};

use crate::tui::{
    styles::FrameType,
    types::{HorizontalLine, Rectangle},
};

use super::{utils::ensure_cursor_at_start, UIElementDraw, UIElementResize};

pub struct Frame<T> {
    area: Rectangle,
    title: String,
    title_len: usize,
    title_style: ContentStyle,
    frame_type: FrameType,
    frame_style: ContentStyle,
    inner: T,
}

impl<T> Frame<T> {
    pub fn new<F: FnOnce(Rectangle) -> T>(
        area: Rectangle,
        title: String,
        title_style: ContentStyle,
        frame_type: FrameType,
        frame_style: ContentStyle,
        inner_builder: F,
    ) -> Self {
        Self {
            area,
            title_len: title.chars().count(),
            title,
            title_style,
            frame_type,
            frame_style,
            inner: inner_builder(area.inside().expect("Frame's area isn't large enough")),
        }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn area(&self) -> Rectangle {
        self.area
    }

    /// Draws the top part of this frame, including the corners and title. This function assumes
    /// that the cursor is positioned at (from_x, self.area.top()), so callers must ensure this.
    fn draw_top<O: Write>(&self, out: &mut O, area: HorizontalLine) -> Result<(), Error> {
        let title_area_width = self.area.width().max(4) - 4;
        let (title_print_length, title_has_three_dots) = if title_area_width < 3 {
            (0, false)
        } else if self.title_len <= title_area_width as usize {
            (self.title_len as u16, false)
        } else {
            (title_area_width - 3, true)
        };

        let title_area_used_width = title_print_length + if title_has_three_dots { 3 } else { 0 };
        let title_area_start = self.area.left() + 2;
        let title_area_end_exclusive = title_area_start + title_area_used_width;

        let mut is_current_style_title = if area.left() >= title_area_start && area.left() < title_area_end_exclusive {
            out.queue(SetStyle(self.title_style))?;
            true
        } else {
            out.queue(SetStyle(self.frame_style))?;
            false
        };

        let mut title_chars_iter = None;

        for x in area.left()..=area.right() {
            let (s, is_title) = if x == self.area.left() {
                (self.frame_type.top_left_corner, false)
            } else if x >= title_area_start && x < title_area_end_exclusive {
                let i = (x - title_area_start) as usize;
                if i < title_print_length as usize {
                    let chars_iter = title_chars_iter.get_or_insert_with(|| self.title.char_indices().skip(i));
                    let (index_at, c) = chars_iter.next().unwrap();
                    (&self.title[index_at..(index_at + c.len_utf8())], true)
                } else {
                    (".", true)
                }
            } else if x == self.area.right() {
                (self.frame_type.top_right_corner, false)
            } else {
                (self.frame_type.horizontal, false)
            };

            if is_current_style_title != is_title {
                is_current_style_title = is_title;
                out.queue(SetStyle(if is_title { self.title_style } else { self.frame_style }))?;
            }

            out.queue(Print(s))?;
        }

        Ok(())
    }

    /// Draws the bottom part of this frame, including the corners. This function assumes that the
    /// cursor is positioned at (from_x, self.area.bottom()), so callers must ensure this.
    fn draw_bottom<O: Write>(&self, out: &mut O, area: HorizontalLine) -> Result<(), Error> {
        out.queue(SetStyle(self.frame_style))?;

        if area.left() == self.area.left() {
            out.queue(Print(self.frame_type.bottom_left_corner))?;
        }

        let horizontal_start_x_inclusive = area.left().max(self.area.left() + 1);
        let horizontal_end_x_noninclusive = area.right().min(self.area.right());

        for _ in horizontal_start_x_inclusive..horizontal_end_x_noninclusive {
            out.queue(Print(self.frame_type.horizontal))?;
        }

        if area.right() == self.area.right() {
            out.queue(Print(self.frame_type.bottom_right_corner))?;
        }

        Ok(())
    }
}

impl<O: Write, T: UIElementDraw<O>> UIElementDraw<O> for Frame<T> {
    fn draw_line(&mut self, out: &mut O, area: HorizontalLine, mut is_cursor_at_start: bool, force_redraw: bool) -> Result<bool, Error> {
        if area.y == self.area.top() {
            if force_redraw {
                ensure_cursor_at_start(&mut is_cursor_at_start, out, area.left(), area.y)?;
                self.draw_top(out, area)?;
            }
        } else if area.y == self.area.bottom() {
            if force_redraw {
                ensure_cursor_at_start(&mut is_cursor_at_start, out, area.left(), area.y)?;
                self.draw_bottom(out, area)?;
            }
        } else {
            if force_redraw && area.left() == self.area.left() {
                ensure_cursor_at_start(&mut is_cursor_at_start, out, area.left(), area.y)?;
                out.queue(PrintStyledContent(StyledContent::new(self.frame_style, self.frame_type.vertical)))?;
            } else {
                is_cursor_at_start = false;
            }

            if let Some(inside) = area.inside() {
                is_cursor_at_start = self.inner.draw_line(out, inside, is_cursor_at_start, force_redraw)?;
            }

            if force_redraw && area.right() == self.area.right() {
                ensure_cursor_at_start(&mut is_cursor_at_start, out, area.right(), area.y)?;
                out.queue(PrintStyledContent(StyledContent::new(self.frame_style, self.frame_type.vertical)))?;
            } else {
                is_cursor_at_start = false;
            }
        }

        Ok(is_cursor_at_start)
    }
}

impl<T: UIElementResize> UIElementResize for Frame<T> {
    fn resize(&mut self, area: Rectangle) {
        self.area = area;
        if let Some(inside) = area.inside() {
            self.inner.resize(inside);
        }
    }
}
