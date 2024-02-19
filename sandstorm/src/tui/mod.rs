use std::{
    io::{stdout, Error, Write},
    rc::Rc,
    time::Duration,
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

use tokio::{io::AsyncWrite, select, sync::Notify};

use crate::{
    printlnif,
    sandstorm::{MutexedSandstormRequestManager, SandstormRequestManager},
    tui::event_receiver::StreamEventReceiver,
};

use self::{event_receiver::TerminalEventReceiver, ui_manager::UIManager};

mod elements;
mod event_receiver;
mod main_view;
mod menu_bar;
mod popups;
mod pretty_print;
mod text_wrapper;
mod ui_element;
mod ui_manager;

pub fn reset_terminal() -> Result<(), Error> {
    stdout()
        .queue(cursor::Show)?
        .queue(event::DisableMouseCapture)?
        .queue(terminal::LeaveAlternateScreen)?
        .flush()?;
    terminal::disable_raw_mode()
}

pub async fn handle_interactive<W>(
    verbose: bool,
    mut manager: SandstormRequestManager<W>,
    terminal_reset_required: &mut bool,
) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + 'static,
{
    printlnif!(verbose, "Starting interactive mode, enabling event stream");

    let (stream_event_receiver, metrics) = StreamEventReceiver::new(&mut manager).await?;
    printlnif!(verbose, "Entering interactive mode");

    let mut out = std::io::stdout();

    *terminal_reset_required = true;
    terminal::enable_raw_mode()?;
    out.queue(terminal::EnterAlternateScreen)?
        .queue(event::EnableMouseCapture)?
        .queue(terminal::SetTitle("Sandstorm Monitor"))?
        .flush()?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    handle_interactive_inner(&mut terminal, manager, stream_event_receiver, metrics).await

    // Note: resetting the terminal is done by the `reset_terminal` function, which gets called in `client.rs`
    // at the end of the `run_client_inner` function. The reason for this is that if any error occurs with the
    // sandstorm connection, this task will be aborted and the error will be handled all the way up there. In
    // other words, we can't handle those errors from here.
}

fn is_ctrl_c(key_event: &KeyEvent) -> bool {
    (key_event.code == KeyCode::Char('c') || key_event.code == KeyCode::Char('C'))
        && key_event.modifiers.contains(KeyModifiers::CONTROL)
        && key_event.kind == KeyEventKind::Press
}

async fn shutdown_task<W>(mut rc: Rc<MutexedSandstormRequestManager<W>>)
where
    W: AsyncWrite + Unpin + 'static,
{
    const RC_UNWRAP_RETRY_FREQUENCY: Duration = Duration::from_millis(200);

    let manager = loop {
        match Rc::try_unwrap(rc) {
            Ok(mutexed) => break mutexed.into_inner(),
            Err(rc_again) => {
                rc = rc_again;
                tokio::time::sleep(RC_UNWRAP_RETRY_FREQUENCY).await;
            }
        }
    };

    let _ = manager.shutdown_and_close().await;
}

async fn handle_interactive_inner<B, W>(
    terminal: &mut Terminal<B>,
    manager: SandstormRequestManager<W>,
    mut stream_event_receiver: StreamEventReceiver,
    metrics: Metrics,
) -> Result<(), Error>
where
    B: Backend,
    W: AsyncWrite + Unpin + 'static,
{
    let manager = Rc::new(manager.into_mutexed());

    let shutdown_notify = Rc::new(Notify::const_new());
    let redraw_notify = Rc::new(Notify::const_new());
    redraw_notify.notify_one();

    let mut terminal_event_receiver = TerminalEventReceiver::new();

    let mut ui_manager = UIManager::new(
        Rc::downgrade(&manager),
        metrics,
        Rc::clone(&redraw_notify),
        Rc::clone(&shutdown_notify),
    );
    let mut manager = Some(manager);

    loop {
        select! {
            biased;
            _ = shutdown_notify.notified() => {
                if let Some(rc) = manager.take() {
                    tokio::task::spawn_local(async move {
                        shutdown_task(rc).await;
                    });
                }
            }
            _ = ui_manager.background_process() => {}
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
                ui_manager.handle_stream_event(stream_event);
            }
            _ = redraw_notify.notified() => {
                terminal.draw(|frame| ui_manager.draw(frame))?;
            }
        }
    }
}
