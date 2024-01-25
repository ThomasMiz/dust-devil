use std::{
    io::{Error, Write},
    num::NonZeroU16,
    time::Duration,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{ContentStyle, Print, Stylize},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use tokio::io::AsyncWrite;

use crate::sandstorm::SandstormRequestManager;

use self::{
    styles::frame_types,
    types::{HorizontalLine, Rectangle},
    ui_elements::{frame::Frame, solid::Solid, UIElementDraw, UIElementResize},
};

mod chars;
mod styles;
mod types;
mod ui_elements;

pub async fn handle_interactive<W>(manager: &mut SandstormRequestManager<W>) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    let mut out = std::io::stdout();

    out.execute(EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    out.execute(EnableMouseCapture)?;

    handle_interactive_inner(&mut out, manager).await?;

    out.execute(DisableMouseCapture)?;
    terminal::disable_raw_mode()?;
    out.execute(LeaveAlternateScreen)?;

    Ok(())
}

async fn handle_interactive_inner<O, W>(out: &mut O, _manager: &mut SandstormRequestManager<W>) -> Result<(), Error>
where
    O: Write,
    W: AsyncWrite + Unpin,
{
    let mut terminal_size = terminal::size()?;
    let mut frame = Frame::new(
        Rectangle::from_borders(0, 0, terminal_size.0 - 1, terminal_size.1 - 1).unwrap(),
        String::from("A ver si funca lcdll↑↓║█◄►←→┃ ━ ┓ ┏ ┛ ┗ ┫ ┣ ┳ ┻ ╋║ ═ ╗ ╔ ╝ ╚ ╣ ╠ ╦ ╩ ╬│ ─ ┐ ╮ ┌ ╭ ┘ ╯ └ ╰ ┤ ├ ┬ ┴ ┼ pedro"),
        ContentStyle::default().red(),
        frame_types::LINE,
        ContentStyle::default().reset(),
        |area| Solid::new(area, "X", ContentStyle::default().blue().on_yellow()),
    );

    for y in 0..terminal_size.1 {
        frame.draw_line(out, HorizontalLine::new(y, frame.area().left(), frame.area().width), false, true)?;
    }
    out.flush()?;

    loop {
        while event::poll(Duration::default())? {
            let event = event::read()?;

            if let Event::Key(e) = event {
                if let KeyCode::Char(c) = e.code {
                    //out.execute(Print(c))?;
                }
            }

            if let Event::Key(key_event) = event {
                if key_event.code == KeyCode::Char('c')
                    && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && key_event.kind == KeyEventKind::Press
                {
                    return Ok(());
                }

                if key_event.code == KeyCode::Esc {
                    return Ok(());
                }
            }

            if let Event::Resize(width, height) = event {
                terminal_size = (width, height);
                frame.resize(Rectangle::from_borders(0, 0, terminal_size.0 - 1, terminal_size.1 - 1).unwrap());
                for y in 0..terminal_size.1 {
                    frame.draw_line(out, HorizontalLine::new(y, frame.area().left(), frame.area().width), false, true)?;
                }
                out.flush()?;
            }
        }
    }
}
