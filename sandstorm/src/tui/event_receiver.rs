use std::{
    io::{Error, ErrorKind},
    thread,
};

use crossterm::event;
use dust_devil_core::{
    logging,
    sandstorm::{EventStreamConfigResponse, EventStreamResponse, Metrics},
};
use tokio::{
    io::AsyncWrite,
    sync::{
        mpsc::{self, Receiver},
        oneshot,
    },
};

use crate::sandstorm::{EventStreamReceiver, SandstormRequestManager};

const EVENT_CHANNEL_SIZE: usize = 32;
const SERVER_EVENTS_CHANNEL_SIZE: usize = 128;

pub struct TerminalEventReceiver {
    receiver: Receiver<Result<event::Event, Error>>,
}

impl TerminalEventReceiver {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(EVENT_CHANNEL_SIZE);

        thread::spawn(move || loop {
            let event = event::read();
            if sender.blocking_send(event).is_err() {
                break;
            }
        });

        Self { receiver }
    }

    pub async fn receive(&mut self) -> Result<Result<event::Event, Error>, TerminalEventReceiveError> {
        self.receiver.recv().await.ok_or(TerminalEventReceiveError)
    }
}

pub struct TerminalEventReceiveError;

impl From<TerminalEventReceiveError> for Error {
    fn from(_value: TerminalEventReceiveError) -> Self {
        Error::new(
            ErrorKind::Other,
            "Failed to receive data from events thread. Did the event reader thread panic?",
        )
    }
}

pub struct StreamEventReceiver {
    receiver: Receiver<EventStreamResponse>,
}

impl StreamEventReceiver {
    pub async fn new<W>(manager: &mut SandstormRequestManager<W>) -> Result<(StreamEventReceiver, Metrics), Error>
    where
        W: AsyncWrite + Unpin,
    {
        let (result_tx, result_rx) = oneshot::channel::<Result<(Metrics, mpsc::Receiver<EventStreamResponse>), Error>>();

        manager
            .event_stream_config_fn(true, move |event_stream_config_result| match event_stream_config_result {
                EventStreamConfigResponse::Enabled(metrics) => {
                    let (event_sender, event_receiver) = mpsc::channel(SERVER_EVENTS_CHANNEL_SIZE);
                    result_tx.send(Ok((metrics, event_receiver))).unwrap();
                    Some(EventStreamReceiver::Channel(event_sender))
                }
                EventStreamConfigResponse::Disabled => {
                    result_tx
                        .send(Err(Error::new(ErrorKind::Other, "Couldn't enable event stream: Server refused")))
                        .unwrap();
                    None
                }
                EventStreamConfigResponse::WasAlreadyEnabled => {
                    result_tx
                        .send(Err(Error::new(
                            ErrorKind::Other,
                            "Couldn't enable event stream: Server responded with \"already enabled\"",
                        )))
                        .unwrap();
                    None
                }
            })
            .await?;
        manager.flush_writer().await?;

        let (metrics, receiver) = result_rx
            .await
            .map_err(|_| Error::new(ErrorKind::Other, "Unknown error while enabling event streaming"))??;

        Ok((Self { receiver }, metrics))
    }

    pub async fn receive(&mut self) -> Result<logging::Event, StreamEventReceiveError> {
        self.receiver.recv().await.map(|response| response.0).ok_or(StreamEventReceiveError)
    }
}

pub struct StreamEventReceiveError;

impl From<StreamEventReceiveError> for Error {
    fn from(_value: StreamEventReceiveError) -> Self {
        Error::new(ErrorKind::Other, "Unknown error while receiving stream event")
    }
}
