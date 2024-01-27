use std::{
    io::{Error, Write},
    time::Duration,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{ContentStyle, Stylize},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use tokio::io::AsyncWrite;

use crate::sandstorm::SandstormRequestManager;

use self::{
    styles::frame_types,
    types::{HorizontalLine, Rectangle},
    ui_elements::{frame::Frame, layouts::vertical_split::VerticalSplit, menu_bar::MenuBar, solid::Solid, UIElement},
};

mod chars;
mod pretty_print;
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

    let mut ui_root = VerticalSplit::new(
        Rectangle::from_borders(0, 0, terminal_size.0 - 1, terminal_size.1 - 1).unwrap(),
        2,
        |upper_area| MenuBar::new(upper_area),
        |bottom_area| {
            Frame::new(
                bottom_area,
                String::from("A ver si funca lcdll↑↓║█◄►←→┃ ━ ┓ ┏ ┛ ┗ ┫ ┣ ┳ ┻ ╋║ ═ ╗ ╔ ╝ ╚ ╣ ╠ ╦ ╩ ╬│ ─ ┐ ╮ ┌ ╭ ┘ ╯ └ ╰ ┤ ├ ┬ ┴ ┼ pedro"),
                ContentStyle::default().red(),
                frame_types::LINE,
                ContentStyle::default().reset(),
                |area| Solid::new(area, "X", ContentStyle::default().blue().on_yellow()),
            )
        },
    );

    for y in 0..terminal_size.1 {
        ui_root.draw_line(
            out,
            HorizontalLine::new(y, ui_root.area().left(), ui_root.area().width),
            false,
            true,
        )?;
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
                ui_root.resize(Rectangle::from_borders(0, 0, terminal_size.0 - 1, terminal_size.1 - 1).unwrap());
                for y in 0..terminal_size.1 {
                    ui_root.draw_line(
                        out,
                        HorizontalLine::new(y, ui_root.area().left(), ui_root.area().width),
                        false,
                        true,
                    )?;
                }
                out.flush()?;
            }
        }
    }
}
