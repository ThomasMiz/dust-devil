use std::{
    io::{Error, Write},
    rc::{Rc, Weak},
    time::Duration,
};

use crossterm::{
    cursor::{self, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{ContentStyle, Stylize},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand, QueueableCommand,
};
use tokio::{io::AsyncWrite, select, sync::Notify};

use crate::sandstorm::SandstormRequestManager;

use self::{
    styles::frame_types,
    types::Rectangle,
    ui_elements::{frame::Frame, layouts::vertical_split::VerticalSplit, menu_bar::MenuBar, solid::Solid, UIElement},
    ui_manager::UIManager,
};

mod chars;
mod pretty_print;
mod styles;
mod types;
mod ui_elements;
mod ui_manager;

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub async fn handle_interactive<W>(manager: &mut SandstormRequestManager<W>) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    let mut out = std::io::stdout();

    terminal::enable_raw_mode()?;
    out.queue(EnterAlternateScreen)?
        .queue(EnableMouseCapture)?
        .queue(cursor::Hide)?
        .flush()?;

    handle_interactive_inner(&mut out, manager).await?;

    out.queue(cursor::Show)?
        .queue(DisableMouseCapture)?
        .queue(LeaveAlternateScreen)?
        .flush()?;
    terminal::disable_raw_mode()?;

    Ok(())
}

fn build_ui_root(area: Rectangle, redraw_notify: &Weak<Notify>) -> impl UIElement {
    VerticalSplit::new(
        area,
        2,
        |upper_area| MenuBar::new(upper_area, Weak::clone(&redraw_notify)),
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
    )
}

async fn handle_interactive_inner<O, W>(out: &mut O, _manager: &mut SandstormRequestManager<W>) -> Result<(), Error>
where
    O: Write,
    W: AsyncWrite + Unpin,
{
    let redraw_notify = Rc::new(Notify::const_new());
    let mut ui_manager = UIManager::new(Rc::downgrade(&redraw_notify), build_ui_root)?;
    redraw_notify.notify_one();

    let mut force_redraw = true;

    ui_manager.draw(out, true)?;

    let mut event_poll_interval = tokio::time::interval(EVENT_POLL_INTERVAL);
    event_poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        select! {
            _ = event_poll_interval.tick() => {
                while event::poll(Duration::from_secs(0))? {
                    let event = event::read()?;

                    if let Event::Key(key_event) = event {
                        if (key_event.code == KeyCode::Char('c') || key_event.code == KeyCode::Char('C'))
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
                        ui_manager.handle_resize(width, height);
                        redraw_notify.notify_one();
                        force_redraw = true;
                    } else {
                        ui_manager.handle_event(event);
                    }
                }
            }
            _ = redraw_notify.notified() => {
                ui_manager.draw(out, force_redraw)?;
                force_redraw = false;
            }
        }
    }
}
