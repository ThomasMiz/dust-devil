use std::{
    io::{stdout, Error, Write},
    rc::Rc,
};

use crossterm::{
    cursor,
    event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal, QueueableCommand,
};
use dust_devil_core::sandstorm::Metrics;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

use tokio::{
    io::AsyncWrite,
    select,
    sync::{oneshot, Notify},
};

use crate::{printlnif, sandstorm::SandstormRequestManager, tui::event_receiver::StreamEventReceiver};

use self::{event_receiver::TerminalEventReceiver, ui_manager::UIManager};

mod event_receiver;
mod menu_bar;
mod pretty_print;
mod ui_manager;

pub async fn handle_interactive<W>(verbose: bool, mut manager: SandstormRequestManager<W>) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + 'static,
{
    printlnif!(verbose, "Starting interactive mode, enabling event stream");

    let (stream_event_receiver, metrics) = StreamEventReceiver::new(&mut manager).await?;
    printlnif!(verbose, "Entering interactive mode");

    let mut out = std::io::stdout();

    terminal::enable_raw_mode()?;
    out.queue(terminal::EnterAlternateScreen)?
        .queue(event::EnableMouseCapture)?
        .queue(cursor::Hide)?
        .flush()?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    handle_interactive_inner(&mut terminal, manager, stream_event_receiver, metrics).await?;
    drop(terminal);

    out.queue(cursor::Show)?
        .queue(event::DisableMouseCapture)?
        .queue(terminal::LeaveAlternateScreen)?
        .flush()?;
    terminal::disable_raw_mode()?;

    Ok(())
}

fn is_ctrl_c(key_event: &KeyEvent) -> bool {
    (key_event.code == KeyCode::Char('c') || key_event.code == KeyCode::Char('C'))
        && key_event.modifiers.contains(KeyModifiers::CONTROL)
        && key_event.kind == KeyEventKind::Press
}

async fn handle_interactive_inner<B, W>(
    terminal: &mut Terminal<B>,
    manager: SandstormRequestManager<W>,
    mut stream_event_receiver: StreamEventReceiver,
    _metrics: Metrics,
) -> Result<(), Error>
where
    B: Backend,
    W: AsyncWrite + Unpin + 'static,
{
    let manager = Rc::new(manager.into_mutexed());

    let (shutdown_sender, mut shutdown_receiver) = oneshot::channel::<()>();
    let redraw_notify = Rc::new(Notify::const_new());
    redraw_notify.notify_one();

    let mut terminal_event_receiver = TerminalEventReceiver::new();

    let mut ui_manager = UIManager::new(manager, Rc::clone(&redraw_notify), shutdown_sender);

    loop {
        select! {
            biased;
            _ = Pin::new(&mut shutdown_receiver) => {
                return Ok(());
            }
            terminal_event_result = terminal_event_receiver.receive() => {
                let terminal_event = terminal_event_result??;
                match terminal_event {
                    event::Event::Resize(_, _) => redraw_notify.notify_one(),
                    event::Event::Key(key_event) if is_ctrl_c(&key_event) => return Ok(()),
                    _ => ui_manager.handle_terminal_event(&terminal_event),
                }
            }
            stream_event_result = stream_event_receiver.receive() => {
                let stream_event = stream_event_result?;
                ui_manager.handle_stream_event(&stream_event);
            }
            _ = redraw_notify.notified() => {
                terminal.draw(|frame| ui_manager.draw(frame))?;
            }
        }
    }
}
