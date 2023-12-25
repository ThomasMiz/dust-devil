use dust_devil_core::logging::LogEvent;
use tokio::{
    sync::mpsc::{self, Sender},
    task::{JoinError, JoinHandle},
};

const EVENT_LOG_BUFFER: usize = 1024;

pub struct LogManager {
    log_sender: Sender<LogEvent>,
    log_task_handle: JoinHandle<()>,
}

impl LogManager {
    pub fn new() -> Self {
        let (log_sender, mut log_receiver) = mpsc::channel::<LogEvent>(EVENT_LOG_BUFFER);

        let log_task_handle = tokio::spawn(async move {
            let mut recv_vec = Vec::with_capacity(16);
            let limit = recv_vec.capacity();
            while log_receiver.recv_many(&mut recv_vec, limit).await != 0 {
                for event in recv_vec.iter() {
                    println!("{}", event);
                }

                recv_vec.clear();
            }
        });

        LogManager {
            log_sender,
            log_task_handle,
        }
    }

    pub fn new_tx(&self) -> Sender<LogEvent> {
        self.log_sender.clone()
    }

    pub async fn join(self) -> Result<(), JoinError> {
        drop(self.log_sender);
        self.log_task_handle.await
    }
}
