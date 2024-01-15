use std::{
    future::{self, Future},
    io::{self, ErrorKind},
    net::SocketAddr,
    ops::DerefMut,
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use dust_devil_core::{
    logging::Event,
    sandstorm::{
        AddSandstormSocketResponse, AddSocks5SocketResponse, CurrentMetricsResponse, EventStreamConfigResponse, EventStreamResponseRef,
        GetBufferSizeResponse, ListAuthMethodsResponse, ListSandstormSocketsResponse, ListSocks5SocketsResponse, ListUsersResponse,
        MeowResponse, Metrics, RemoveSandstormSocketResponse, RemoveSocketResponse, RemoveSocks5SocketResponse, SetBufferSizeResponse,
        ShutdownRequest, ToggleAuthMethodResponse,
    },
    serialize::ByteWrite,
};

use tokio::{
    io::{AsyncWrite, AsyncWriteExt, BufWriter},
    select,
    sync::{broadcast, mpsc, oneshot},
};

use crate::context::SandstormContext;

use super::{error_handling::ToIoResult, messaging::ResponseNotification};

/// The maximum amount requests for a specific stream type (e.g. "socks5 sockets requests") that
/// can pile up before the response handler starts waiting on them to be completed synchronously.
///
/// This doesn't need to be a high value; why would anyone be asking to list/add/remove a socks5 or
/// sandstorm sockets over and over again repeatedly without delay? 4 is more than enough.
const RECEIVER_BUFFER_SIZE: usize = 4;

enum SocketRequestReceiver {
    List(oneshot::Receiver<Vec<SocketAddr>>),
    Add(oneshot::Receiver<Result<(), io::Error>>),
    Remove(oneshot::Receiver<RemoveSocketResponse>),
}

enum SocketRequestResult {
    List(Vec<SocketAddr>),
    Add(Result<(), io::Error>),
    Remove(RemoveSocketResponse),
}

/// Awaiting a SocketRequestReceiver will return a SocketRequestResult with the result of calling
/// `.await` on the oneshot::Receiver contaiend within the enum variant of the same name.
impl Future for SocketRequestReceiver {
    type Output = Result<SocketRequestResult, oneshot::error::RecvError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match self.deref_mut() {
            Self::List(receiver) => Pin::new(receiver).poll(cx).map(|r| r.map(SocketRequestResult::List)),
            Self::Add(receiver) => Pin::new(receiver).poll(cx).map(|r| r.map(SocketRequestResult::Add)),
            Self::Remove(receiver) => Pin::new(receiver).poll(cx).map(|r| r.map(SocketRequestResult::Remove)),
        }
    }
}

/// If the vector is empty, returns a future that stalls indefinitely. Otherwise, waits for the
/// first future in the vector to complete, removes it from the vector, and returns its result.
async fn get_first_if<F: Future<Output = R> + Unpin, R>(receivers: &mut Vec<F>) -> R {
    if let Some(receiver) = receivers.first_mut() {
        let result = receiver.await;
        receivers.remove(0);
        result
    } else {
        future::pending().await
    }
}

/// If the vector is empty, returns a future that stalls indefinitely. If the first element of the
/// vector is None, completes with Ok(None) immediately. If the first element of the vector is
/// Some(receiver), then waits for the receiver to receive a value (if it hasn't already done so)
/// and completes with Ok(Some()) containing said received value. Whenever a None is found in the
/// vector or a receiver completes, it gets taken out of the vector.
///
/// This is a wrapper function that simplifies processing receivers in a vector, in the order in
/// which they are in the vector.
async fn get_first_optional<T>(receivers: &mut Vec<Option<oneshot::Receiver<T>>>) -> Result<Option<T>, oneshot::error::RecvError> {
    let maybe_receiver = match receivers.first_mut() {
        Some(r) => r,
        None => future::pending().await,
    };

    match maybe_receiver {
        Some(receiver) => {
            let result = receiver.await.map(|v| Some(v));
            receivers.remove(0);
            result
        }
        None => {
            receivers.remove(0);
            Ok(None)
        }
    }
}

/// If `maybe_receiver` is `Some(receiver)`, then this function returns the same as calling
/// `receiver.recv()`. Otherwise, if `None`, returns a future that never completes. This function
/// is intended for easy use within a `select!` block, as adding a
/// `value = recv_if_some(&mut receiver) => {...}` to the `select!` will create a branch that only
/// execute if the receiver is Some() and something has been received.
async fn recv_if_some<T: Clone>(maybe_receiver: &mut Option<broadcast::Receiver<T>>) -> Result<T, broadcast::error::RecvError> {
    if let Some(receiver) = maybe_receiver {
        receiver.recv().await
    } else {
        future::pending().await
    }
}

/// Completes immediately if the writer's buffer isn't empty, otherwise stalls indefinitely.
///
/// This is useful because `BufWriter` doesn't flush unless it gets full or is told to flush. That
/// means that if we write a response to a message we'd need to flush it immediately, as otherwise
/// that response might wait indefinitely to be sent!
///
/// However, flushing after each response might be very inefficient, specially given that most
/// responses are pretty small, some even a single byte. Also, what if we receive, in a single
/// read, three requests from a client and want to send back the three responses in a single write?
///
/// This helper function makes handling that easy. Instead of flushing the buffer after each write,
/// place the future generated by this function as the last branch in a biased `select!` block like
/// so:
///
/// ```
/// loop {
///     select! {
///         biased;
///         ... // handle messages & responses, write things to the writer
///         _ = stall_if_empty(writer) => writer.flush().await,
///     }
/// }
/// ````
///
/// And now the writer will be automatically flushed when it's not empty, and there's nothing else
/// that needs writing to it.
async fn stall_if_empty<W>(writer: &BufWriter<W>)
where
    W: AsyncWrite + Unpin,
{
    if writer.buffer().is_empty() {
        future::pending().await
    }
}

struct ResponseHandlerState {
    socks_receivers: Vec<SocketRequestReceiver>,
    sandstorm_receivers: Vec<SocketRequestReceiver>,
    metrics_receivers: Vec<Option<oneshot::Receiver<Metrics>>>,
    event_stream_receiver: Option<broadcast::Receiver<Arc<Event>>>,
}

pub async fn handle_responses<W>(
    writer: &mut BufWriter<W>,
    context: &SandstormContext,
    mut response_notifier: mpsc::Receiver<ResponseNotification>,
) -> Result<(), io::Error>
where
    W: AsyncWrite + Unpin,
{
    let mut handler_state = ResponseHandlerState {
        socks_receivers: Vec::with_capacity(RECEIVER_BUFFER_SIZE),
        sandstorm_receivers: Vec::with_capacity(RECEIVER_BUFFER_SIZE),
        metrics_receivers: Vec::with_capacity(RECEIVER_BUFFER_SIZE),
        event_stream_receiver: None,
    };

    loop {
        select! {
            biased;
            maybe_notification = response_notifier.recv() => {
                let notification = match maybe_notification {
                    Some(notif) => notif,
                    None => return Ok(()),
                };

                handle_notification(
                    notification,
                    writer,
                    context,
                    &mut handler_state
                ).await?;
            }
            result = get_first_if(&mut handler_state.socks_receivers) => {
                let socket_result = result.map_err_to_io()?;
                handle_socks5_response(socket_result, writer).await?;
            }
            result = get_first_if(&mut handler_state.sandstorm_receivers) => {
                let socket_result = result.map_err_to_io()?;
                handle_sandstorm_response(socket_result, writer).await?;
            }
            result = get_first_optional(&mut handler_state.metrics_receivers) => {
                let result = result.map_err_to_io()?;
                CurrentMetricsResponse(result).write(writer).await?;
            }
            maybe_event = recv_if_some(&mut handler_state.event_stream_receiver) => {
                match maybe_event {
                    Ok(evt) => EventStreamResponseRef(evt.as_ref()).write(writer).await?,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                    Err(broadcast::error::RecvError::Lagged(count)) => return Err(io::Error::new(ErrorKind::Other, format!("Connection too slow to stream events, lagged behind {count} events!"))),
                }
            }
            _ = stall_if_empty(writer) => writer.flush().await?,
        }
    }
}

async fn handle_socks5_response<W>(socket_result: SocketRequestResult, writer: &mut W) -> Result<(), io::Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    match socket_result {
        SocketRequestResult::List(result) => ListSocks5SocketsResponse(result).write(writer).await,
        SocketRequestResult::Add(result) => AddSocks5SocketResponse(result).write(writer).await,
        SocketRequestResult::Remove(result) => RemoveSocks5SocketResponse(result).write(writer).await,
    }
}

async fn handle_sandstorm_response<W>(socket_result: SocketRequestResult, writer: &mut W) -> Result<(), io::Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    match socket_result {
        SocketRequestResult::List(result) => ListSandstormSocketsResponse(result).write(writer).await,
        SocketRequestResult::Add(result) => AddSandstormSocketResponse(result).write(writer).await,
        SocketRequestResult::Remove(result) => RemoveSandstormSocketResponse(result).write(writer).await,
    }
}

async fn handle_notification<W>(
    notification: ResponseNotification,
    writer: &mut W,
    context: &SandstormContext,
    handler_state: &mut ResponseHandlerState,
) -> Result<(), io::Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    match notification {
        ResponseNotification::Shutdown(receiver) => {
            let _ = receiver.await;
            ShutdownRequest.write(writer).await?;
        }
        ResponseNotification::LogEventConfig(enabled) => {
            let result = match (enabled, &handler_state.event_stream_receiver) {
                (false, _) => {
                    handler_state.event_stream_receiver.take();
                    EventStreamConfigResponse::Disabled
                }
                (true, Some(_)) => EventStreamConfigResponse::WasAlreadyEnabled,
                (true, None) => match context.request_metrics_and_subscribe().await {
                    Some(result_receiver) => {
                        let (metrics, event_receiver) = result_receiver.await.map_err_to_io()?;
                        handler_state.event_stream_receiver = Some(event_receiver);
                        EventStreamConfigResponse::Enabled(metrics)
                    }
                    None => EventStreamConfigResponse::Disabled,
                },
            };

            result.write(writer).await?;
        }
        ResponseNotification::ListSocks5Sockets(receiver) => {
            if handler_state.socks_receivers.len() == handler_state.socks_receivers.capacity() {
                let result = handler_state.socks_receivers.remove(0).await.map_err_to_io()?;
                handle_socks5_response(result, writer).await?;
            }
            handler_state.socks_receivers.push(SocketRequestReceiver::List(receiver));
        }
        ResponseNotification::AddSocks5Socket(receiver) => {
            if handler_state.socks_receivers.len() == handler_state.socks_receivers.capacity() {
                let result = handler_state.socks_receivers.remove(0).await.map_err_to_io()?;
                handle_socks5_response(result, writer).await?;
            }
            handler_state.socks_receivers.push(SocketRequestReceiver::Add(receiver));
        }
        ResponseNotification::RemoveSocks5Socket(receiver) => {
            if handler_state.socks_receivers.len() == handler_state.socks_receivers.capacity() {
                let result = handler_state.socks_receivers.remove(0).await.map_err_to_io()?;
                handle_socks5_response(result, writer).await?;
            }
            handler_state.socks_receivers.push(SocketRequestReceiver::Remove(receiver));
        }
        ResponseNotification::ListSandstormSockets(receiver) => {
            if handler_state.sandstorm_receivers.len() == handler_state.sandstorm_receivers.capacity() {
                let result = handler_state.sandstorm_receivers.remove(0).await.map_err_to_io()?;
                handle_sandstorm_response(result, writer).await?;
            }
            handler_state.sandstorm_receivers.push(SocketRequestReceiver::List(receiver));
        }
        ResponseNotification::AddSandstormSocket(receiver) => {
            if handler_state.sandstorm_receivers.len() == handler_state.sandstorm_receivers.capacity() {
                let result = handler_state.sandstorm_receivers.remove(0).await.map_err_to_io()?;
                handle_sandstorm_response(result, writer).await?;
            }
            handler_state.sandstorm_receivers.push(SocketRequestReceiver::Add(receiver));
        }
        ResponseNotification::RemoveSandstormSocket(receiver) => {
            if handler_state.sandstorm_receivers.len() == handler_state.sandstorm_receivers.capacity() {
                let result = handler_state.sandstorm_receivers.remove(0).await.map_err_to_io()?;
                handle_sandstorm_response(result, writer).await?;
            }
            handler_state.sandstorm_receivers.push(SocketRequestReceiver::Remove(receiver));
        }
        ResponseNotification::ListUsers(snapshot) => {
            ListUsersResponse(snapshot).write(writer).await?;
        }
        ResponseNotification::AddUser(result) => {
            result.write(writer).await?;
        }
        ResponseNotification::UpdateUser(result) => {
            result.write(writer).await?;
        }
        ResponseNotification::DeleteUser(result) => {
            result.write(writer).await?;
        }
        ResponseNotification::ListAuthMethods(auth_methods) => {
            ListAuthMethodsResponse(auth_methods).write(writer).await?;
        }
        ResponseNotification::ToggleAuthMethod(result) => {
            ToggleAuthMethodResponse(result).write(writer).await?;
        }
        ResponseNotification::RequestCurrentMetrics(maybe_receiver) => {
            if handler_state.metrics_receivers.len() == handler_state.metrics_receivers.capacity() {
                let maybe_receiver = handler_state.metrics_receivers.remove(0);
                let result = match maybe_receiver {
                    Some(receiver) => Some(receiver.await.map_err_to_io()?),
                    None => None,
                };
                CurrentMetricsResponse(result).write(writer).await?;
            }

            handler_state.metrics_receivers.push(maybe_receiver);
        }
        ResponseNotification::GetBufferSize(buffer_size) => {
            GetBufferSizeResponse(buffer_size).write(writer).await?;
        }
        ResponseNotification::SetBufferSize(result) => {
            SetBufferSizeResponse(result).write(writer).await?;
            writer.flush().await?;
        }
        ResponseNotification::Meow => {
            MeowResponse.write(writer).await?;
        }
    }

    Ok(())
}
