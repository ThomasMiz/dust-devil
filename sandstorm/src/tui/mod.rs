use std::{
    io::{stdout, Error, Write},
    rc::Rc,
    time::Duration,
};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal, QueueableCommand,
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::Rect,
    Terminal,
};

use tokio::{io::AsyncWrite, select, sync::Notify};

use crate::{sandstorm::SandstormRequestManager, tui::menu_bar::MenuBar};

use self::ui_manager::UIManager;

mod menu_bar;
mod pretty_print;
mod ui_manager;

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub async fn handle_interactive<W>(manager: &mut SandstormRequestManager<W>) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    let mut out = std::io::stdout();

    terminal::enable_raw_mode()?;
    out.queue(terminal::EnterAlternateScreen)?
        .queue(event::EnableMouseCapture)?
        .queue(cursor::Hide)?
        .flush()?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    handle_interactive_inner(&mut terminal, manager).await?;
    drop(terminal);

    out.queue(cursor::Show)?
        .queue(event::DisableMouseCapture)?
        .queue(terminal::LeaveAlternateScreen)?
        .flush()?;
    terminal::disable_raw_mode()?;

    Ok(())
}

async fn handle_interactive_inner<B, W>(terminal: &mut Terminal<B>, _manager: &mut SandstormRequestManager<W>) -> Result<(), Error>
where
    B: Backend,
    W: AsyncWrite + Unpin,
{
    let redraw_notify = Rc::new(Notify::const_new());
    redraw_notify.notify_one();

    let mut event_poll_interval = tokio::time::interval(EVENT_POLL_INTERVAL);
    event_poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut ui_manager = UIManager::new(Rc::clone(&redraw_notify));

    loop {
        select! {
            biased;
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

                    if let Event::Resize(_, _) = event {
                        redraw_notify.notify_one();
                    } else {
                        // TODO: Handle events
                    }
                }
            }
            _ = redraw_notify.notified() => {
                terminal.draw(|frame| ui_manager.draw(frame))?;
            }
        }
    }
}
