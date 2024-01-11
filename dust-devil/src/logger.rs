use std::{io, path::PathBuf, sync::Arc};

use dust_devil_core::logging::{LogEvent, LogEventType};
use time::{OffsetDateTime, UtcOffset};
use tokio::{
    fs::File,
    io::{stdout, AsyncWrite, AsyncWriteExt, BufWriter},
    sync::broadcast::{self, error::RecvError, Receiver, Sender},
    task::{JoinError, JoinHandle},
};

use std::io::Write;

use crate::printlnif;

const EVENT_LOG_BUFFER: usize = 4096;
const STDOUT_BUFFER_SIZE: usize = 0x2000;
const FILE_BUFFER_SIZE: usize = 0x2000;
const PARSE_VEC_SIZE: usize = 0x100;

pub struct LogManager {
    log_sender: Sender<Arc<LogEvent>>,
    log_stdout_task_handle: Option<JoinHandle<()>>,
    log_file_task_handle: Option<JoinHandle<()>>,
}

pub struct LogSender {
    log_sender: Sender<Arc<LogEvent>>,
}

async fn logger_task<W>(
    verbose: bool,
    mut log_receiver: Receiver<Arc<LogEvent>>,
    utc_offset: UtcOffset,
    writer: &mut W,
    name: &str,
) -> Result<(), io::Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    let mut parse_vec = Vec::<u8>::with_capacity(PARSE_VEC_SIZE);

    loop {
        match log_receiver.recv().await {
            Ok(event) => {
                let t = OffsetDateTime::from_unix_timestamp(event.timestamp)
                    .map(|t| t.to_offset(utc_offset))
                    .unwrap_or(OffsetDateTime::UNIX_EPOCH);

                let _ = writeln!(
                    parse_vec,
                    "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}] {}",
                    t.year(),
                    t.month() as u8,
                    t.day(),
                    t.hour(),
                    t.minute(),
                    t.second(),
                    event.data
                );
            }
            Err(RecvError::Lagged(lost_count)) => {
                let _ = writeln!(parse_vec, "ERROR!! {lost_count} events lost due to slowdown!");
            }
            Err(RecvError::Closed) => break,
        }

        writer.write_all(&parse_vec).await?;
        parse_vec.clear();

        if log_receiver.is_empty() {
            writer.flush().await?;
        }
    }

    printlnif!(verbose, "Logger task for {name} exited event receiving loop, shutting down");

    writer.shutdown().await?;
    Ok(())
}

async fn logger_task_wrapper<W>(verbose: bool, log_receiver: Receiver<Arc<LogEvent>>, utc_offset: UtcOffset, mut writer: W, name: &str)
where
    W: AsyncWrite + Unpin,
{
    printlnif!(verbose, "Logger task for {name} started");

    match logger_task(verbose, log_receiver, utc_offset, &mut writer, name).await {
        Ok(()) => printlnif!(verbose, "Logger task for {name} finished"),
        Err(error) => eprintln!("Logger task for {name} finished with error: {error}"),
    }
}

fn setup_logger_tasks(
    verbose: bool,
    log_receiver: Receiver<Arc<LogEvent>>,
    log_to_stdout: bool,
    file: Option<File>,
) -> (Option<JoinHandle<()>>, Option<JoinHandle<()>>) {
    let local_utc_offset = match UtcOffset::current_local_offset() {
        Ok(offset) => {
            printlnif!(verbose, "Local UTC offset determined to be at {offset}");
            offset
        }
        Err(_) => {
            eprintln!("Could not determine system's UTC offset, defaulting to 00:00:00");
            UtcOffset::UTC
        }
    };

    // Note: `Stdout` is already buffered, as it's wrapped in a `LineWriter` that internally uses a `BufWriter`
    // (not the tokio one, the std one). However, this buffer is (currently) only 1024 bytes.
    let maybe_stdout_task_handle = if log_to_stdout {
        let stdout_writer = BufWriter::with_capacity(STDOUT_BUFFER_SIZE, stdout());
        let log_receiver1 = log_receiver.resubscribe();
        let log_stdout_task_handle = tokio::spawn(async move {
            logger_task_wrapper(verbose, log_receiver1, local_utc_offset, stdout_writer, "stdout").await;
        });
        Some(log_stdout_task_handle)
    } else {
        None
    };

    let maybe_file_task_handle = file.map(|f| {
        let file_writer = BufWriter::with_capacity(FILE_BUFFER_SIZE, f);
        tokio::spawn(async move {
            logger_task_wrapper(verbose, log_receiver, local_utc_offset, file_writer, "file").await;
        })
    });

    (maybe_stdout_task_handle, maybe_file_task_handle)
}

async fn create_file(verbose: bool, path: &str) -> Option<File> {
    if let Some(parent) = PathBuf::from(path).parent() {
        printlnif!(verbose, "Creating directory for logger file: {parent:?}");
        if let Err(error) = tokio::fs::DirBuilder::new().recursive(true).create(parent).await {
            eprintln!("Failed to create directory for log file {path}: {error}");
            return None;
        }
    }

    printlnif!(verbose, "Logger opening up file: {path}");
    let file_result = File::options().write(true).read(false).create(true).append(true).open(path).await;

    match file_result {
        Ok(f) => {
            printlnif!(verbose, "Logger successfully opened up file: {path}");
            Some(f)
        }
        Err(err) => {
            eprintln!("Failed to open log file {path}: {err}");
            None
        }
    }
}

impl LogManager {
    pub async fn new(verbose: bool, log_to_stdout: bool, log_to_file: Option<&str>) -> Self {
        let (log_sender, log_receiver) = broadcast::channel::<Arc<LogEvent>>(EVENT_LOG_BUFFER);

        let file = if let Some(path) = log_to_file {
            create_file(verbose, path).await
        } else {
            printlnif!(
                verbose,
                "Logger doesn't have stdout nor file enabled. Yet you did turn on verbose prints? The audacity."
            );

            None
        };

        let (log_stdout_task_handle, log_file_task_handle) = setup_logger_tasks(verbose, log_receiver, log_to_stdout, file);

        LogManager {
            log_sender,
            log_stdout_task_handle,
            log_file_task_handle,
        }
    }

    pub fn new_sender(&self) -> LogSender {
        LogSender::new(self.log_sender.clone())
    }

    pub async fn join(self) -> Result<(), JoinError> {
        drop(self.log_sender);

        if let Some(handle) = self.log_stdout_task_handle {
            handle.await?;
        }

        if let Some(handle) = self.log_file_task_handle {
            handle.await?;
        }

        Ok(())
    }
}

impl LogSender {
    fn new(log_sender: Sender<Arc<LogEvent>>) -> Self {
        LogSender { log_sender }
    }

    pub fn send(&self, data: LogEventType) -> bool {
        let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
        self.log_sender.send(Arc::new(LogEvent::new(timestamp, data))).is_ok()
    }
}
