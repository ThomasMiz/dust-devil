use std::{io::Error, path::PathBuf, sync::Arc};

use dust_devil_core::{
    logging::{Event, EventData},
    sandstorm::Metrics,
};
use time::{OffsetDateTime, UtcOffset};
use tokio::{
    fs::File,
    io::{stdout, AsyncWrite, AsyncWriteExt, BufWriter},
    select,
    sync::{
        broadcast::{self, error::RecvError, Receiver, Sender},
        mpsc, oneshot,
    },
    task::{JoinError, JoinHandle},
};

use std::io::Write;

use crate::printlnif;

const EVENT_LOG_BUFFER: usize = 0x1000;
const STDOUT_BUFFER_SIZE: usize = 0x2000;
const FILE_BUFFER_SIZE: usize = 0x2000;
const PARSE_VEC_SIZE: usize = 0x100;
const METRICS_REQUEST_CHANNEL_SIZE: usize = 0x10;

pub struct LogManager {
    log_sender: Sender<Arc<Event>>,
    metrics_request_sender: mpsc::Sender<MetricsRequest>,
    metrics_task_handle: JoinHandle<()>,
    log_stdout_task_handle: Option<JoinHandle<()>>,
    log_file_task_handle: Option<JoinHandle<()>>,
}

pub struct LogSender {
    log_sender: Sender<Arc<Event>>,
}

enum MetricsRequest {
    Metrics(oneshot::Sender<Metrics>),
    MetricsAndSubscribe(oneshot::Sender<(Metrics, Receiver<Arc<Event>>)>),
}

pub struct MetricsRequester {
    request_sender: mpsc::Sender<MetricsRequest>,
}

async fn logger_task<W>(
    verbose: bool,
    mut log_receiver: Receiver<Arc<Event>>,
    utc_offset: UtcOffset,
    writer: &mut W,
    name: &str,
) -> Result<(), Error>
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

async fn logger_task_wrapper<W>(verbose: bool, log_receiver: Receiver<Arc<Event>>, utc_offset: UtcOffset, mut writer: W, name: &str)
where
    W: AsyncWrite + Unpin,
{
    printlnif!(verbose, "Logger task for {name} started");

    match logger_task(verbose, log_receiver, utc_offset, &mut writer, name).await {
        Ok(()) => printlnif!(verbose, "Logger task for {name} finished"),
        Err(error) => eprintln!("Logger task for {name} finished with error: {error}"),
    }
}

async fn metrics_task(verbose: bool, mut log_receiver: Receiver<Arc<Event>>, mut request_receiver: mpsc::Receiver<MetricsRequest>) {
    printlnif!(verbose, "Metrics tracker task started");

    let mut current_client_connections: u32 = 0;
    let mut historic_client_connections: u64 = 0;
    let mut client_bytes_sent: u64 = 0;
    let mut client_bytes_received: u64 = 0;
    let mut current_sandstorm_connections: u32 = 0;
    let mut historic_sandstorm_connections: u64 = 0;

    loop {
        select! {
            biased;
            event = log_receiver.recv() => {
                let event = match event {
                    Ok(evt) => evt,
                    Err(RecvError::Lagged(amount)) => {
                        eprintln!("Warning! Metrics tracker lagged behind {amount} events!");
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                };

                match event.data {
                    EventData::NewClientConnectionAccepted(_, _) => {
                        current_client_connections += 1;
                        historic_client_connections += 1;
                    }
                    EventData::ClientConnectionFinished(_, _, _, _) => {
                        current_client_connections -= 1;
                    }
                    EventData::ClientBytesSent(_, count) => {
                        client_bytes_sent += count;
                    }
                    EventData::ClientBytesReceived(_, count) => {
                        client_bytes_received += count;
                    }
                    EventData::NewSandstormConnectionAccepted(_, _) => {
                        current_sandstorm_connections += 1;
                        historic_sandstorm_connections += 1;
                    }
                    EventData::SandstormConnectionFinished(_, _) => {
                        current_sandstorm_connections -= 1;
                    }
                    _ => {}
                }
            }
            request = request_receiver.recv() => {
                let request = match request {
                    Some(req) => req,
                    None => break,
                };

                let metrics = Metrics {
                    current_client_connections,
                    historic_client_connections,
                    client_bytes_sent,
                    client_bytes_received,
                    current_sandstorm_connections,
                    historic_sandstorm_connections,
                };

                match request {
                    MetricsRequest::Metrics(sender) => {
                        let _ = sender.send(metrics);
                    },
                    MetricsRequest::MetricsAndSubscribe(sender) => {
                        // Note: log_receiver.resubcribe() makes a new receiver that receives values _after_ the resubcribe.
                        // This means, in the Sandstorm protocol, the metrics sent won't be truly synchronized with the event
                        // stream. Note however that this branch is last in a biased select! block, and therefore won't
                        // execute if the previous branch completes immediately. This means this branch doesn't run unless
                        // the log_receiver is empty! There is still, however, a very slight chance that a log event comes in
                        // right in between the start of this branch and the `.resubscribe()`.

                        // Since the option to fully fix this would be to have a second broadcast channel that is synchronized
                        // with the events, and all events are relayed from the original broadcast to this second broadcast by
                        // this task, for performance reasons it makes more sense to just accept this minor issue.
                        let _ = sender.send((metrics, log_receiver.resubscribe()));
                    }
                }
            }
        }
    }

    printlnif!(verbose, "Metrics tracker task finished");
}

fn setup_logger_tasks(
    verbose: bool,
    log_receiver: Receiver<Arc<Event>>,
    metrics_request_receiver: mpsc::Receiver<MetricsRequest>,
    log_to_stdout: bool,
    file: Option<File>,
) -> (JoinHandle<()>, Option<JoinHandle<()>>, Option<JoinHandle<()>>) {
    let log_receiver1 = log_receiver.resubscribe();
    let metrics_tracker_task = tokio::spawn(async move {
        metrics_task(verbose, log_receiver1, metrics_request_receiver).await;
    });

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

    (metrics_tracker_task, maybe_stdout_task_handle, maybe_file_task_handle)
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
        let (log_sender, log_receiver) = broadcast::channel::<Arc<Event>>(EVENT_LOG_BUFFER);

        let file = if let Some(path) = log_to_file {
            create_file(verbose, path).await
        } else {
            printlnif!(
                !log_to_stdout && verbose,
                "Logger doesn't have stdout nor file enabled. Yet you did turn on verbose prints? The audacity."
            );

            None
        };

        let (metrics_request_sender, metrics_request_receiver) = mpsc::channel(METRICS_REQUEST_CHANNEL_SIZE);

        let (metrics_task_handle, log_stdout_task_handle, log_file_task_handle) =
            setup_logger_tasks(verbose, log_receiver, metrics_request_receiver, log_to_stdout, file);

        LogManager {
            log_sender,
            metrics_request_sender,
            metrics_task_handle,
            log_stdout_task_handle,
            log_file_task_handle,
        }
    }

    pub fn new_sender(&self) -> LogSender {
        LogSender::new(self.log_sender.clone())
    }

    pub fn new_requester(&self) -> MetricsRequester {
        MetricsRequester::new(self.metrics_request_sender.clone())
    }

    pub async fn join(self) -> Result<(), JoinError> {
        drop(self.log_sender);
        drop(self.metrics_request_sender);

        self.metrics_task_handle.await?;

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
    fn new(log_sender: Sender<Arc<Event>>) -> Self {
        LogSender { log_sender }
    }

    pub fn send(&self, data: EventData) -> bool {
        let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
        self.log_sender.send(Arc::new(Event::new(timestamp, data))).is_ok()
    }
}

impl MetricsRequester {
    fn new(request_sender: mpsc::Sender<MetricsRequest>) -> MetricsRequester {
        MetricsRequester { request_sender }
    }

    pub async fn request_metrics(&self) -> Option<oneshot::Receiver<Metrics>> {
        let (result_tx, result_rx) = oneshot::channel();

        let result = self.request_sender.send(MetricsRequest::Metrics(result_tx)).await;
        match result {
            Ok(()) => Some(result_rx),
            Err(_) => None,
        }
    }

    pub async fn request_metrics_and_subscribe(&self) -> Option<oneshot::Receiver<(Metrics, broadcast::Receiver<Arc<Event>>)>> {
        let (result_tx, result_rx) = oneshot::channel();

        let result = self.request_sender.send(MetricsRequest::MetricsAndSubscribe(result_tx)).await;
        match result {
            Ok(()) => Some(result_rx),
            Err(_) => None,
        }
    }
}
