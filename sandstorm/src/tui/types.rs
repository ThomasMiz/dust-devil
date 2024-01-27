use std::{
    num::NonZeroU16,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

impl Point {
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

impl From<(u16, u16)> for Point {
    fn from(value: (u16, u16)) -> Self {
        Self { x: value.0, y: value.1 }
    }
}

impl From<u16> for Point {
    fn from(value: u16) -> Self {
        Self { x: value, y: value }
    }
}

impl From<Point> for (u16, u16) {
    fn from(value: Point) -> Self {
        (value.x, value.y)
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl SubAssign for Point {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul for Point {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}

impl MulAssign for Point {
    fn mul_assign(&mut self, rhs: Self) {
        self.x *= rhs.x;
        self.y *= rhs.y;
    }
}

impl Div for Point {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

impl DivAssign for Point {
    fn div_assign(&mut self, rhs: Self) {
        self.x /= rhs.x;
        self.y /= rhs.y;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    pub top_left: Point,
    pub width: NonZeroU16,
    pub height: NonZeroU16,
}

/// An axis-aligned rectangle in a 2D grid of pixels with u16 coordinates. Guaranteed to not be
/// empty (contains at least 1 pixel).
impl Rectangle {
    pub const fn new(top_left: Point, width: NonZeroU16, height: NonZeroU16) -> Self {
        Self { top_left, width, height }
    }

    /// Creates a new rectangle from the desired borders (included).
    pub const fn from_borders(left: u16, top: u16, right: u16, bottom: u16) -> Option<Self> {
        if right >= left && bottom >= top {
            let top_left = Point::new(left, top);
            let width = unsafe { NonZeroU16::new_unchecked(right + 1 - left) };
            let height = unsafe { NonZeroU16::new_unchecked(bottom + 1 - top) };

            Some(Self { top_left, width, height })
        } else {
            None
        }
    }

    /// Gets this rectangle's width. Guaranteed to not be zero.
    pub const fn width(&self) -> u16 {
        self.width.get()
    }

    /// Gets this rectangle's height. Guaranteed to not be zero.
    pub const fn height(&self) -> u16 {
        self.height.get()
    }

    /// Gets this rectangle's lowest inclusive X coordinate.
    pub const fn left(&self) -> u16 {
        self.top_left.x
    }

    /// Gets this rectangle's highest inclusive X coordinate. Guaranteed to be `>= left()`.
    pub const fn right(&self) -> u16 {
        self.top_left.x + self.width() - 1
    }

    /// Gets this rectangle's lowest inclusive Y coordinate.
    pub const fn top(&self) -> u16 {
        self.top_left.y
    }

    /// Gets this rectangle's highest inclusive Y coordinate. Guaranteed to be `>= top()`.
    pub const fn bottom(&self) -> u16 {
        self.top_left.y + self.height() - 1
    }

    pub const fn bottom_left(&self) -> Point {
        Point {
            x: self.left(),
            y: self.bottom(),
        }
    }

    pub const fn top_right(&self) -> Point {
        Point {
            x: self.right(),
            y: self.top(),
        }
    }

    pub const fn bottom_right(&self) -> Point {
        Point {
            x: self.right(),
            y: self.bottom(),
        }
    }

    /// Gets this rectangle's area.
    pub const fn area(&self) -> u16 {
        self.width() * self.height()
    }

    /// Calculates the intersection between this rectangle and another.
    pub fn intersection_with(&self, other: Self) -> Option<Self> {
        let left = self.left().max(other.left());
        let right = self.right().min(other.right());
        let top = self.top().max(other.top());
        let bottom = self.bottom().min(other.bottom());

        Self::from_borders(left, top, right, bottom)
    }

    /// Calculates the inside of this rectangle by pushing each border inwards by 1 unit.
    pub fn inside(&self) -> Option<Self> {
        if self.width() > 2 && self.height() > 2 {
            let top_left = self.top_left + 1.into();
            let width = unsafe { NonZeroU16::new_unchecked(self.width() - 2) };
            let height = unsafe { NonZeroU16::new_unchecked(self.height() - 2) };

            Some(Self { top_left, width, height })
        } else {
            None
        }
    }

    pub fn top_as_line(&self) -> HorizontalLine {
        HorizontalLine::new(self.top(), self.left(), self.width)
    }

    pub const fn get_single_pixel_rect() -> Self {
        let one = unsafe { NonZeroU16::new_unchecked(1) };
        Rectangle::new(Point::new(0, 0), one, one)
    }
}

/// A Horizontal line of pixels in a 2D grid. Guaranteed to not be empty (contains at least 1 pixel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HorizontalLine {
    pub y: u16,
    pub left: u16,
    pub width: NonZeroU16,
}

impl HorizontalLine {
    pub const fn new(y: u16, left: u16, width: NonZeroU16) -> Self {
        Self { y, left, width }
    }

    /// Creates a new line from the desired borders (included). Returns None if `left > right`.
    pub const fn from_borders(y: u16, left: u16, right: u16) -> Option<Self> {
        if left <= right {
            let width = unsafe { NonZeroU16::new_unchecked(right - left + 1) };
            Some(HorizontalLine { y, left, width })
        } else {
            None
        }
    }

    /// Gets this line's lowest inclusive X coordinate.
    pub const fn left(&self) -> u16 {
        self.left
    }

    /// Gets this line's highest inclusive X coordinate. Guaranteed to be `>= left()`.
    pub const fn right(&self) -> u16 {
        self.left + self.width.get() - 1
    }

    /// Gets this line's width. Guaranteed to not be zero.
    pub const fn width(&self) -> u16 {
        self.width.get()
    }

    /// Calculates the intersection between this line and a rectangle.
    pub fn intersection_with_rect(&self, rect: Rectangle) -> Option<Self> {
        let left = self.left().max(rect.left());
        let right = self.right().min(rect.right());

        if self.y >= rect.top() && self.y <= rect.bottom() {
            Self::from_borders(self.y, left, right)
        } else {
            None
        }
    }

    /// Calculates the intersection between this line and another line.
    pub fn intersection_with_line(&self, line: Self) -> Option<Self> {
        if self.y == line.y {
            let left = self.left().max(line.left());
            let right = self.right().min(line.right());
            Self::from_borders(self.y, left, right)
        } else {
            None
        }
    }

    /// Calculates the inside of this line by pushing the horizontal borders inwards by 1 unit.
    pub const fn inside(&self) -> Option<Self> {
        if self.width() > 2 {
            Some(Self {
                y: self.y,
                left: self.left + 1,
                width: unsafe { NonZeroU16::new_unchecked(self.width() - 2) },
            })
        } else {
            None
        }
    }

    pub const fn get_single_pixel_line() -> Self {
        let one = unsafe { NonZeroU16::new_unchecked(1) };
        Self::new(0, 0, one)
    }
}

impl From<HorizontalLine> for Rectangle {
    fn from(value: HorizontalLine) -> Self {
        let one = unsafe { NonZeroU16::new_unchecked(1) };
        Rectangle::new(Point::new(value.left, value.y), value.width, one)
    }
}

#[cfg(test)]
mod tests {
    use crate::tui::types::Rectangle;

    #[test]
    fn test_rectangle_intersection1() {
        assert_eq!(
            Rectangle::from_borders(10, 10, 20, 20),
            Rectangle::from_borders(0, 0, 20, 20)
                .unwrap()
                .intersection_with(Rectangle::from_borders(10, 10, 30, 30).unwrap())
        );

        assert_eq!(
            Rectangle::from_borders(10, 10, 20, 20),
            Rectangle::from_borders(10, 10, 30, 30)
                .unwrap()
                .intersection_with(Rectangle::from_borders(0, 0, 20, 20).unwrap())
        );
    }

    #[test]
    fn test_rectangle_intersection2() {
        assert_eq!(
            Rectangle::from_borders(10, 10, 20, 20),
            Rectangle::from_borders(10, 10, 20, 20)
                .unwrap()
                .intersection_with(Rectangle::from_borders(0, 0, 50, 50).unwrap())
        );

        assert_eq!(
            Rectangle::from_borders(10, 10, 20, 20),
            Rectangle::from_borders(0, 0, 50, 50)
                .unwrap()
                .intersection_with(Rectangle::from_borders(10, 10, 20, 20).unwrap())
        );
    }
}
