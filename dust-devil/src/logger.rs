use std::io;

use dust_devil_core::logging::{LogEvent, LogEventType};
use time::{OffsetDateTime, UtcOffset};
use tokio::{
    fs::File,
    io::{stdout, AsyncWrite, AsyncWriteExt, BufWriter},
    join,
    sync::mpsc::{
        self,
        error::{SendError, TrySendError},
        Receiver, Sender,
    },
    task::{JoinError, JoinHandle},
};

use std::io::Write;

use crate::printlnif;

const EVENT_LOG_BUFFER: usize = 1024;
const STDOUT_BUFFER_SIZE: usize = 0x2000;
const FILE_BUFFER_SIZE: usize = 0x2000;
const RECV_VEC_SIZE: usize = 0x40;
const PARSE_VEC_SIZE: usize = 0x100;

pub struct LogManager {
    log_sender: Sender<LogEvent>,
    log_task_handle: JoinHandle<()>,
}

pub struct LogSender {
    log_sender: Sender<LogEvent>,
}

async fn write_if_some<W: AsyncWrite + Unpin>(writer_name: &str, maybe_writer: &mut Option<W>, buf: &[u8]) {
    if let Some(w) = maybe_writer {
        if let Err(error) = w.write_all(buf).await {
            eprintln!("Error while writing logs to {writer_name}: {error}");
            *maybe_writer = None;
        }
    }
}

async fn flush_if_some<W: AsyncWrite + Unpin>(writer_name: &str, maybe_writer: &mut Option<W>) {
    if let Some(w) = maybe_writer {
        if let Err(error) = w.flush().await {
            eprintln!("Error while flushing logs to {writer_name}: {error}");
            *maybe_writer = None;
        }
    }
}

async fn shutdown_if_some<W: AsyncWrite + Unpin>(writer_name: &str, maybe_writer: &mut Option<W>) {
    if let Some(w) = maybe_writer {
        if let Err(error) = w.shutdown().await {
            eprintln!("Error while flushing logs to {writer_name}: {error}");
            *maybe_writer = None;
        }
    }
}

async fn logger_task(verbose: bool, mut log_receiver: Receiver<LogEvent>, log_to_stdout: bool, file: Option<File>) {
    printlnif!(verbose, "Logger task started");

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
    let mut maybe_stdout = if log_to_stdout {
        Some(BufWriter::with_capacity(STDOUT_BUFFER_SIZE, stdout()))
    } else {
        None
    };

    let mut maybe_file = file.map(|f| BufWriter::with_capacity(FILE_BUFFER_SIZE, f));

    let mut recv_vec = Vec::with_capacity(RECV_VEC_SIZE);
    let mut parse_vec = Vec::<u8>::with_capacity(PARSE_VEC_SIZE);
    let limit = recv_vec.capacity();

    printlnif!(verbose, "Logger task entering event receiving loop");

    while log_receiver.recv_many(&mut recv_vec, limit).await != 0 {
        for event in recv_vec.iter() {
            let t = OffsetDateTime::from_unix_timestamp(event.timestamp)
                .map(|t| t.to_offset(local_utc_offset))
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

            join!(
                write_if_some("stdout", &mut maybe_stdout, &parse_vec),
                write_if_some("file", &mut maybe_file, &parse_vec),
            );

            parse_vec.clear();
        }

        join!(flush_if_some("stdout", &mut maybe_stdout), flush_if_some("file", &mut maybe_file));

        recv_vec.clear();
    }

    printlnif!(verbose, "Logger task exited event receiving loop, shutting down");

    join!(
        shutdown_if_some("stdout", &mut maybe_stdout),
        shutdown_if_some("file", &mut maybe_file)
    );

    printlnif!(verbose, "Logger task finished");
}

impl LogManager {
    pub async fn new(verbose: bool, log_to_stdout: bool, log_to_file: Option<&str>) -> (Self, Result<(), io::Error>) {
        let (log_sender, log_receiver) = mpsc::channel::<LogEvent>(EVENT_LOG_BUFFER);

        let (file, file_result) = if let Some(path) = log_to_file {
            printlnif!(verbose, "Logger opening up file: {path}");
            let file_result = File::options().write(true).read(false).create(true).append(true).open(path).await;
            match file_result {
                Ok(f) => {
                    printlnif!(verbose, "Logger successfully opened up file: {path}");
                    (Some(f), Ok(()))
                }
                Err(err) => {
                    printlnif!(verbose, "Logger failed to open up file: {path}");
                    (None, Err(err))
                }
            }
        } else {
            printlnif!(
                verbose,
                "Logger doesn't have stdout nor file enabled. Yet you did turn on verbose prints? The audacity."
            );

            (None, Ok(()))
        };

        let log_task_handle = tokio::spawn(async move {
            logger_task(verbose, log_receiver, log_to_stdout, file).await;
        });

        let logger = LogManager {
            log_sender,
            log_task_handle,
        };

        (logger, file_result)
    }

    pub fn new_sender(&self) -> LogSender {
        LogSender::new(self.log_sender.clone())
    }

    pub async fn join(self) -> Result<(), JoinError> {
        drop(self.log_sender);
        self.log_task_handle.await
    }
}

impl LogSender {
    fn new(log_sender: Sender<LogEvent>) -> Self {
        LogSender { log_sender }
    }

    pub async fn send(&self, data: LogEventType) -> Result<(), SendError<LogEvent>> {
        let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
        self.log_sender.send(LogEvent::new(timestamp, data)).await
    }

    /*pub async fn send_timeout(&self, data: LogEventType, timeout: Duration) -> Result<(), SendTimeoutError<LogEvent>> {
        let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
        self.log_sender.send_timeout(LogEvent::new(timestamp, data), timeout).await
    }*/

    pub fn try_send(&self, data: LogEventType) -> Result<(), TrySendError<LogEvent>> {
        let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
        self.log_sender.try_send(LogEvent::new(timestamp, data))
    }

    /*pub fn blocking_send(&self, data: LogEventType) -> Result<(), SendError<LogEvent>> {
        let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
        self.log_sender.blocking_send(LogEvent::new(timestamp, data))
    }*/
}
