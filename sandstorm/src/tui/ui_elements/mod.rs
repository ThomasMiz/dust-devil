use std::io::{Error, Write};

use super::types::{HorizontalLine, Rectangle};

pub mod frame;
pub mod layouts;
pub mod menu_bar;
pub mod simple_label;
pub mod solid;
pub mod utils;

/// An UI element in the TUI that occupies an area on the screen.
pub trait UIElement {
    /// Gets the area on screen this UI element occupies.
    fn area(&self) -> Rectangle;

    /// Draws an horizontal line of chars from this UI element. Drawing occurs by lines to reduce
    /// the amount of cursor movements, since printing a char advances the cursor forward. The area
    /// to draw is defined by the `area` parameter, of type `HorizontalLine`. This area must be
    /// fully within the area of the element, defined by the last call to `resize()`.
    ///
    /// The cursor is not guaranteed to be at the start of the area line to draw, but it might be.
    /// This is indicated by the `is_cursor_at_start` boolean parameter. No guarantees are made
    /// about the style or colors when this function is called.
    ///
    /// If an element has already been drawn to the screen, but part of that element needs an
    /// update, then it'd be inefficient to redraw the entire element. For this, we have the
    /// `force_redraw` parameter. If true, elements should draw themselves entirely from scratch.
    /// Otherwise, elements can assume the contents of the previous draw call are still visible,
    /// and therefore may only update the pixels that needs changing.
    ///
    /// # Return value
    /// If an error is encountered while writing to the output (`out` parameter), then `Err()` is
    /// returned with said error. Otherwise, `Ok()` is returned, with `true` if the cursor was left
    /// positioned at the end of the area line (x = area.right() + 1), and false otherwise. This
    /// function must not change the cursor's Y position.
    ///
    /// # Why lines?
    /// Originally, drawing was done with rectangles. However, this causes inefficiencies with, for
    /// example, a `Frame` containing another UI element, since the frame would have to draw itself
    /// first, and then ask the inner element to draw itself. This means the vertical lines of the
    /// frame would need a lot of cursor movements! When drawing by lines, this allows the `Frame`
    /// to draw, for each line, the left-side vertical border (e.g. '|'), then call the inner's
    /// draw_line(), and then draw the right-side vertical border, thus reducing cursor movements.
    fn draw_line<O: Write>(
        &mut self,
        out: &mut O,
        area: HorizontalLine,
        is_cursor_at_start: bool,
        force_redraw: bool,
    ) -> Result<bool, Error>;

    /// Sets the area on screen this UI element occupies.
    fn resize(&mut self, area: Rectangle);
}
