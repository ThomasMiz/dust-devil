use std::{
    io::{Error, ErrorKind, Write},
    num::NonZeroU16,
    rc::Weak,
};

use crossterm::{event::Event, terminal};
use tokio::sync::Notify;

use super::{
    types::{HorizontalLine, Point, Rectangle},
    ui_elements::{HandleEventResult, UIElement},
};

pub struct UIManager<R: UIElement> {
    root: R,
    redraw_notify: Weak<Notify>,
    terminal_size: Point,
}

impl<R: UIElement> UIManager<R> {
    pub fn new<RB: FnOnce(Rectangle, &Weak<Notify>) -> R>(redraw_notify: Weak<Notify>, root_builder: RB) -> Result<Self, Error> {
        let terminal_size: Point = terminal::size()?.into();

        let terminal_area = match (NonZeroU16::new(terminal_size.x), NonZeroU16::new(terminal_size.y)) {
            (Some(width), Some(height)) => Rectangle::new(Point::new(0, 0), width, height),
            _ => return Err(Error::new(ErrorKind::Other, "Terminal reported a size of 0")),
        };

        let root = root_builder(terminal_area, &redraw_notify);
        Ok(Self {
            root,
            redraw_notify,
            terminal_size,
        })
    }

    pub fn handle_event(&mut self, event: Event) -> HandleEventResult {
        self.root.handle_event(&event)
    }

    pub fn handle_resize(&mut self, width: u16, height: u16) {
        self.terminal_size = Point::new(width, height);
        if let Some(root_area) = Rectangle::from_borders(0, 0, width - 1, height - 1) {
            self.root.resize(root_area);
        }
    }

    pub fn draw<O: Write>(&mut self, out: &mut O, force_redraw: bool) -> Result<(), Error> {
        for y in 0..self.terminal_size.y {
            self.root.draw_line(
                out,
                HorizontalLine::new(y, self.root.area().left(), self.root.area().width),
                false,
                force_redraw,
            )?;
        }
        out.flush()
    }
}
