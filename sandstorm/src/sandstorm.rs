use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{Error, ErrorKind},
    net::SocketAddr,
    ops::Deref,
    rc::Rc,
};

use dust_devil_core::{
    sandstorm::{
        AddSandstormSocketRequest, AddSandstormSocketResponse, AddSocks5SocketRequest, AddSocks5SocketResponse, AddUserRequestRef,
        AddUserResponse, CurrentMetricsRequest, CurrentMetricsResponse, DeleteUserRequestRef, DeleteUserResponse, EventStreamConfigRequest,
        EventStreamConfigResponse, EventStreamResponse, GetBufferSizeRequest, GetBufferSizeResponse, ListAuthMethodsRequest,
        ListAuthMethodsResponse, ListSandstormSocketsRequest, ListSandstormSocketsResponse, ListSocks5SocketsRequest,
        ListSocks5SocketsResponse, ListUsersRequest, ListUsersResponse, MeowRequest, MeowResponse, RemoveSandstormSocketRequest,
        RemoveSandstormSocketResponse, RemoveSocks5SocketRequest, RemoveSocks5SocketResponse, SandstormCommandType, SetBufferSizeRequest,
        SetBufferSizeResponse, ShutdownRequest, ShutdownResponse, ToggleAuthMethodRequest, ToggleAuthMethodResponse, UpdateUserRequestRef,
        UpdateUserResponse,
    },
    serialize::{ByteRead, ByteWrite},
    socks5::AuthMethod,
    users::UserRole,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    sync::{mpsc, oneshot, Mutex},
    task::JoinHandle,
};

pub enum EventStreamReceiver {
    Function(Box<dyn FnMut(EventStreamResponse)>),
    Channel(mpsc::Sender<EventStreamResponse>),
}

struct ResponseHandlers {
    remaining: usize,
    was_shutdown: bool,
    flush_notifier: Option<oneshot::Sender<()>>,
    shutdown_handlers: VecDeque<Box<dyn FnOnce(ShutdownResponse)>>,
    event_stream_config_handlers: VecDeque<Box<dyn FnOnce(EventStreamConfigResponse) -> Option<EventStreamReceiver>>>,
    list_socks5_handlers: VecDeque<Box<dyn FnOnce(ListSocks5SocketsResponse)>>,
    add_socks5_handlers: VecDeque<Box<dyn FnOnce(AddSocks5SocketResponse)>>,
    remove_socks5_handlers: VecDeque<Box<dyn FnOnce(RemoveSocks5SocketResponse)>>,
    list_sandstorm_handlers: VecDeque<Box<dyn FnOnce(ListSandstormSocketsResponse)>>,
    add_sandstorm_handlers: VecDeque<Box<dyn FnOnce(AddSandstormSocketResponse)>>,
    remove_sandstorm_handlers: VecDeque<Box<dyn FnOnce(RemoveSandstormSocketResponse)>>,
    list_users_handlers: VecDeque<Box<dyn FnOnce(ListUsersResponse)>>,
    add_user_handlers: VecDeque<Box<dyn FnOnce(AddUserResponse)>>,
    update_user_handlers: VecDeque<Box<dyn FnOnce(UpdateUserResponse)>>,
    delete_user_handlers: VecDeque<Box<dyn FnOnce(DeleteUserResponse)>>,
    list_auth_methods_handlers: VecDeque<Box<dyn FnOnce(ListAuthMethodsResponse)>>,
    toggle_auth_method_handlers: VecDeque<Box<dyn FnOnce(ToggleAuthMethodResponse)>>,
    get_metrics_handlers: VecDeque<Box<dyn FnOnce(CurrentMetricsResponse)>>,
    get_buffer_size_handlers: VecDeque<Box<dyn FnOnce(GetBufferSizeResponse)>>,
    set_buffer_size_handlers: VecDeque<Box<dyn FnOnce(SetBufferSizeResponse)>>,
    meow_handlers: VecDeque<Box<dyn FnOnce(MeowResponse)>>,
}

pub struct SandstormRequestManager<W>
where
    W: AsyncWrite + Unpin,
{
    writer: W,
    reader_task_handle: JoinHandle<()>,
    handlers: Rc<RefCell<ResponseHandlers>>,
}

async fn reader_task<R>(mut reader: R, read_error_sender: oneshot::Sender<Result<(), Error>>, handlers: Rc<RefCell<ResponseHandlers>>) -> R
where
    R: AsyncRead + Unpin,
{
    let _ = read_error_sender.send(reader_task_inner(&mut reader, handlers).await);
    reader
}

async fn reader_task_inner<R>(reader: &mut R, handlers: Rc<RefCell<ResponseHandlers>>) -> Result<(), Error>
where
    R: AsyncRead + Unpin + ?Sized,
{
    let mut event_stream_sender = None;

    loop {
        let command = match SandstormCommandType::read(reader).await {
            Ok(cmd) => cmd,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        };

        match command {
            SandstormCommandType::Shutdown => {
                let result = ShutdownResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.shutdown_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected Shutdown response")),
                }
            }
            SandstormCommandType::EventStreamConfig => {
                let result = EventStreamConfigResponse::read(reader).await?;

                let mut handlers = handlers.deref().borrow_mut();
                let handler = match handlers.event_stream_config_handlers.pop_front() {
                    Some(f) => f,
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected EventStreamConfig response")),
                };
                handlers.remaining -= 1;
                drop(handlers);

                event_stream_sender = handler(result);
            }
            SandstormCommandType::EventStream => {
                let event = EventStreamResponse::read(reader).await?;
                if let Some(sender) = &mut event_stream_sender {
                    match sender {
                        EventStreamReceiver::Function(f) => f(event),
                        EventStreamReceiver::Channel(s) => {
                            if s.send(event).await.is_err() {
                                event_stream_sender = None;
                            }
                        }
                    }
                }
            }
            SandstormCommandType::ListSocks5Sockets => {
                let result = ListSocks5SocketsResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.list_socks5_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected ListSocks5Sockets response")),
                }
            }
            SandstormCommandType::AddSocks5Socket => {
                let result = AddSocks5SocketResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.add_socks5_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected AddSocks5Socket response")),
                }
            }
            SandstormCommandType::RemoveSocks5Socket => {
                let result = RemoveSocks5SocketResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.remove_socks5_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            "Received unexpected RemoveSocks5Socket response",
                        ))
                    }
                }
            }
            SandstormCommandType::ListSandstormSockets => {
                let result = ListSandstormSocketsResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.list_sandstorm_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            "Received unexpected ListSandstormSockets response",
                        ))
                    }
                }
            }
            SandstormCommandType::AddSandstormSocket => {
                let result = AddSandstormSocketResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.add_sandstorm_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            "Received unexpected AddSandstormSocket response",
                        ))
                    }
                }
            }
            SandstormCommandType::RemoveSandstormSocket => {
                let result = RemoveSandstormSocketResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.remove_sandstorm_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            "Received unexpected RemoveSandstormSocket response",
                        ))
                    }
                }
            }
            SandstormCommandType::ListUsers => {
                let result = ListUsersResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.list_users_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected ListUsers response")),
                }
            }
            SandstormCommandType::AddUser => {
                let result = AddUserResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.add_user_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected AddUser response")),
                }
            }
            SandstormCommandType::UpdateUser => {
                let result = UpdateUserResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.update_user_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected UpdateUser response")),
                }
            }
            SandstormCommandType::DeleteUser => {
                let result = DeleteUserResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.delete_user_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected DeleteUser response")),
                }
            }
            SandstormCommandType::ListAuthMethods => {
                let result = ListAuthMethodsResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.list_auth_methods_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected ListAuthMethods response")),
                }
            }
            SandstormCommandType::ToggleAuthMethod => {
                let result = ToggleAuthMethodResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.toggle_auth_method_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected ToggleAuthMethod response")),
                }
            }
            SandstormCommandType::RequestCurrentMetrics => {
                let result = CurrentMetricsResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.get_metrics_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            "Received unexpected RequestCurrentMetrics response",
                        ))
                    }
                }
            }
            SandstormCommandType::GetBufferSize => {
                let result = GetBufferSizeResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.get_buffer_size_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected GetBufferSize response")),
                }
            }
            SandstormCommandType::SetBufferSize => {
                let result = SetBufferSizeResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.set_buffer_size_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected SetBufferSize response")),
                }
            }
            SandstormCommandType::Meow => {
                let result = MeowResponse::read(reader).await?;
                let mut handlers = handlers.deref().borrow_mut();
                match handlers.meow_handlers.pop_front() {
                    Some(f) => {
                        handlers.remaining -= 1;
                        drop(handlers);
                        f(result);
                    }
                    None => return Err(Error::new(ErrorKind::InvalidData, "Received unexpected Meow response")),
                }
            }
        }

        let mut handlers = handlers.deref().borrow_mut();
        if handlers.remaining == 0 {
            if let Some(sender) = handlers.flush_notifier.take() {
                let _ = sender.send(());
            }
        }
    }

    let handlers = handlers.deref().borrow_mut();
    if handlers.was_shutdown {
        match handlers.remaining {
            0 => Ok(()),
            rem if rem == handlers.shutdown_handlers.len() => Ok(()),
            _ => Err(Error::new(
                ErrorKind::UnexpectedEof,
                "The server closed the connection before answering all the requests",
            )),
        }
    } else {
        Err(Error::new(ErrorKind::ConnectionReset, "The server closed unexpectedly"))
    }
}

#[allow(clippy::await_holding_refcell_ref)] // TODO: Remove once clippy false positive is fixed (https://github.com/rust-lang/rust-clippy/issues/6353)

impl<W> SandstormRequestManager<W>
where
    W: AsyncWrite + Unpin,
{
    pub fn new<R>(reader: R, writer: W) -> (Self, oneshot::Receiver<Result<(), Error>>)
    where
        R: AsyncRead + Unpin + 'static,
    {
        let (read_error_sender, read_error_rx) = oneshot::channel();

        let handlers = Rc::new(RefCell::new(ResponseHandlers {
            remaining: 0,
            was_shutdown: false,
            flush_notifier: None,
            shutdown_handlers: VecDeque::new(),
            event_stream_config_handlers: VecDeque::new(),
            list_socks5_handlers: VecDeque::new(),
            add_socks5_handlers: VecDeque::new(),
            remove_socks5_handlers: VecDeque::new(),
            list_sandstorm_handlers: VecDeque::new(),
            add_sandstorm_handlers: VecDeque::new(),
            remove_sandstorm_handlers: VecDeque::new(),
            list_users_handlers: VecDeque::new(),
            add_user_handlers: VecDeque::new(),
            update_user_handlers: VecDeque::new(),
            delete_user_handlers: VecDeque::new(),
            list_auth_methods_handlers: VecDeque::new(),
            toggle_auth_method_handlers: VecDeque::new(),
            get_metrics_handlers: VecDeque::new(),
            get_buffer_size_handlers: VecDeque::new(),
            set_buffer_size_handlers: VecDeque::new(),
            meow_handlers: VecDeque::new(),
        }));

        let handlers1 = handlers.clone();
        let reader_task_handle = tokio::task::spawn_local(async move {
            reader_task(reader, read_error_sender, handlers1).await;
        });

        let value = Self {
            writer,
            reader_task_handle,
            handlers,
        };

        (value, read_error_rx)
    }

    pub async fn flush_writer(&mut self) -> Result<(), Error> {
        self.writer.flush().await
    }

    pub async fn flush_and_wait(&mut self) -> Result<(), Error> {
        self.writer.flush().await?;
        let mut handlers = self.handlers.deref().borrow_mut();
        if handlers.remaining != 0 {
            let (tx, rx) = oneshot::channel();
            handlers.flush_notifier = Some(tx);
            drop(handlers);
            let _ = rx.await;
        }
        Ok(())
    }

    /// Flushes any remaining requests, indicates to the server that no more requests will be made,
    /// and closes the connection.
    ///
    /// # This future never completes!
    /// Instead, the completion should be handled through the oneshot channel returned by the
    /// constructor of this [`SandstormRequestManager`]. If all goes well, an Ok(()) will be
    /// received.
    pub async fn shutdown_and_close(mut self) -> Result<(), Error> {
        self.handlers.borrow_mut().was_shutdown = true;

        self.writer.shutdown().await?;
        let _ = self.reader_task_handle.await;
        std::future::pending::<()>().await;
        Ok(())
    }

    pub async fn shutdown_fn<F: FnOnce(ShutdownResponse) + 'static>(&mut self, f: F) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.shutdown_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        handlers.was_shutdown = true;
        drop(handlers);
        ShutdownRequest.write(&mut self.writer).await
    }

    pub async fn event_stream_config_fn<F: FnOnce(EventStreamConfigResponse) -> Option<EventStreamReceiver> + 'static>(
        &mut self,
        status: bool,
        f: F,
    ) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.event_stream_config_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        EventStreamConfigRequest(status).write(&mut self.writer).await
    }

    pub async fn list_socks5_sockets_fn<F: FnOnce(ListSocks5SocketsResponse) + 'static>(&mut self, f: F) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.list_socks5_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        ListSocks5SocketsRequest.write(&mut self.writer).await
    }

    pub async fn add_socks5_socket_fn<F: FnOnce(AddSocks5SocketResponse) + 'static>(
        &mut self,
        address: SocketAddr,
        f: F,
    ) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.add_socks5_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        AddSocks5SocketRequest(address).write(&mut self.writer).await
    }

    pub async fn remove_socks5_socket_fn<F: FnOnce(RemoveSocks5SocketResponse) + 'static>(
        &mut self,
        address: SocketAddr,
        f: F,
    ) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.remove_socks5_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        RemoveSocks5SocketRequest(address).write(&mut self.writer).await
    }

    pub async fn list_sandstorm_sockets_fn<F: FnOnce(ListSandstormSocketsResponse) + 'static>(&mut self, f: F) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.list_sandstorm_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        ListSandstormSocketsRequest.write(&mut self.writer).await
    }

    pub async fn add_sandstorm_socket_fn<F: FnOnce(AddSandstormSocketResponse) + 'static>(
        &mut self,
        address: SocketAddr,
        f: F,
    ) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.add_sandstorm_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        AddSandstormSocketRequest(address).write(&mut self.writer).await
    }

    pub async fn remove_sandstorm_socket_fn<F: FnOnce(RemoveSandstormSocketResponse) + 'static>(
        &mut self,
        address: SocketAddr,
        f: F,
    ) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.remove_sandstorm_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        RemoveSandstormSocketRequest(address).write(&mut self.writer).await
    }

    pub async fn list_users_fn<F: FnOnce(ListUsersResponse) + 'static>(&mut self, f: F) -> Result<(), Error> {
        self.handlers.deref().borrow_mut().list_users_handlers.push_back(Box::new(f));
        ListUsersRequest.write(&mut self.writer).await
    }

    pub async fn add_user_fn<F: FnOnce(AddUserResponse) + 'static>(
        &mut self,
        username: &str,
        password: &str,
        role: UserRole,
        f: F,
    ) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.add_user_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        AddUserRequestRef(username, password, role).write(&mut self.writer).await
    }

    pub async fn update_user_fn<F: FnOnce(UpdateUserResponse) + 'static>(
        &mut self,
        username: &str,
        password: Option<&str>,
        role: Option<UserRole>,
        f: F,
    ) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.update_user_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        UpdateUserRequestRef(username, password, role).write(&mut self.writer).await
    }

    pub async fn delete_user_fn<F: FnOnce(DeleteUserResponse) + 'static>(&mut self, username: &str, f: F) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.delete_user_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        DeleteUserRequestRef(username).write(&mut self.writer).await
    }

    pub async fn list_auth_methods_fn<F: FnOnce(ListAuthMethodsResponse) + 'static>(&mut self, f: F) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.list_auth_methods_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        ListAuthMethodsRequest.write(&mut self.writer).await
    }

    pub async fn toggle_auth_method_fn<F: FnOnce(ToggleAuthMethodResponse) + 'static>(
        &mut self,
        auth_method: AuthMethod,
        status: bool,
        f: F,
    ) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.toggle_auth_method_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        ToggleAuthMethodRequest(auth_method, status).write(&mut self.writer).await
    }

    pub async fn get_metrics_fn<F: FnOnce(CurrentMetricsResponse) + 'static>(&mut self, f: F) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.get_metrics_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        CurrentMetricsRequest.write(&mut self.writer).await
    }

    pub async fn get_buffer_size_fn<F: FnOnce(GetBufferSizeResponse) + 'static>(&mut self, f: F) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.get_buffer_size_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        GetBufferSizeRequest.write(&mut self.writer).await
    }

    pub async fn set_buffer_size_fn<F: FnOnce(SetBufferSizeResponse) + 'static>(&mut self, buffer_size: u32, f: F) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.set_buffer_size_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        SetBufferSizeRequest(buffer_size).write(&mut self.writer).await
    }

    pub async fn meow_fn<F: FnOnce(MeowResponse) + 'static>(&mut self, f: F) -> Result<(), Error> {
        let mut handlers = self.handlers.deref().borrow_mut();
        handlers.meow_handlers.push_back(Box::new(f));
        handlers.remaining += 1;
        drop(handlers);
        MeowRequest.write(&mut self.writer).await
    }

    pub fn into_mutexed(self) -> MutexedSandstormRequestManager<W> {
        MutexedSandstormRequestManager { inner: Mutex::new(self) }
    }
}

/// A [`SandstormRequestManager`] wrapped in an async [`Mutex`], for easier use in concurrent
/// environments. Provides the same operations but with `&self` instead of `&mut self`, and
/// automatically flushes the writer after each request.
pub struct MutexedSandstormRequestManager<W: AsyncWrite + Unpin> {
    inner: Mutex<SandstormRequestManager<W>>,
}

impl<W: AsyncWrite + Unpin> MutexedSandstormRequestManager<W> {
    pub fn into_inner(self) -> SandstormRequestManager<W> {
        self.inner.into_inner()
    }

    pub async fn shutdown_fn<F: FnOnce(ShutdownResponse) + 'static>(&self, f: F) -> Result<(), Error> {
        let mut guard = self.inner.lock().await;
        guard.shutdown_fn(f).await?;
        guard.flush_writer().await
    }

    // pub async fn list_socks5_sockets_fn<F: FnOnce(ListSocks5SocketsResponse) + 'static>(&self, f: F) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.list_socks5_sockets_fn(f).await?;
    //     guard.flush_writer().await
    // }

    // pub async fn add_socks5_socket_fn<F: FnOnce(AddSocks5SocketResponse) + 'static>(&self, address: SocketAddr, f: F) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.add_socks5_socket_fn(address, f).await?;
    //     guard.flush_writer().await
    // }

    // pub async fn remove_socks5_socket_fn<F: FnOnce(RemoveSocks5SocketResponse) + 'static>(
    //     &self,
    //     address: SocketAddr,
    //     f: F,
    // ) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.remove_socks5_socket_fn(address, f).await?;
    //     guard.flush_writer().await
    // }

    // pub async fn list_sandstorm_sockets_fn<F: FnOnce(ListSandstormSocketsResponse) + 'static>(&self, f: F) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.list_sandstorm_sockets_fn(f).await?;
    //     guard.flush_writer().await
    // }

    // pub async fn add_sandstorm_socket_fn<F: FnOnce(AddSandstormSocketResponse) + 'static>(
    //     &self,
    //     address: SocketAddr,
    //     f: F,
    // ) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.add_sandstorm_socket_fn(address, f).await?;
    //     guard.flush_writer().await
    // }

    // pub async fn remove_sandstorm_socket_fn<F: FnOnce(RemoveSandstormSocketResponse) + 'static>(
    //     &self,
    //     address: SocketAddr,
    //     f: F,
    // ) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.remove_sandstorm_socket_fn(address, f).await?;
    //     guard.flush_writer().await
    // }

    // pub async fn list_users_fn<F: FnOnce(ListUsersResponse) + 'static>(&self, f: F) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.list_users_fn(f).await?;
    //     guard.flush_writer().await
    // }

    // pub async fn add_user_fn<F: FnOnce(AddUserResponse) + 'static>(
    //     &self,
    //     username: &str,
    //     password: &str,
    //     role: UserRole,
    //     f: F,
    // ) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.add_user_fn(username, password, role, f).await?;
    //     guard.flush_writer().await
    // }

    // pub async fn update_user_fn<F: FnOnce(UpdateUserResponse) + 'static>(
    //     &self,
    //     username: &str,
    //     password: Option<&str>,
    //     role: Option<UserRole>,
    //     f: F,
    // ) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.update_user_fn(username, password, role, f).await?;
    //     guard.flush_writer().await
    // }

    // pub async fn delete_user_fn<F: FnOnce(DeleteUserResponse) + 'static>(&self, username: &str, f: F) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.delete_user_fn(username, f).await?;
    //     guard.flush_writer().await
    // }

    pub async fn list_auth_methods_fn<F: FnOnce(ListAuthMethodsResponse) + 'static>(&self, f: F) -> Result<(), Error> {
        let mut guard = self.inner.lock().await;
        guard.list_auth_methods_fn(f).await?;
        guard.flush_writer().await
    }

    pub async fn toggle_auth_method_fn<F: FnOnce(ToggleAuthMethodResponse) + 'static>(
        &self,
        auth_method: AuthMethod,
        status: bool,
        f: F,
    ) -> Result<(), Error> {
        let mut guard = self.inner.lock().await;
        guard.toggle_auth_method_fn(auth_method, status, f).await?;
        guard.flush_writer().await
    }

    // pub async fn get_metrics_fn<F: FnOnce(CurrentMetricsResponse) + 'static>(&self, f: F) -> Result<(), Error> {
    //     let mut guard = self.inner.lock().await;
    //     guard.get_metrics_fn(f).await?;
    //     guard.flush_writer().await
    // }

    pub async fn get_buffer_size_fn<F: FnOnce(GetBufferSizeResponse) + 'static>(&self, f: F) -> Result<(), Error> {
        let mut guard = self.inner.lock().await;
        guard.get_buffer_size_fn(f).await?;
        guard.flush_writer().await
    }

    pub async fn set_buffer_size_fn<F: FnOnce(SetBufferSizeResponse) + 'static>(&self, buffer_size: u32, f: F) -> Result<(), Error> {
        let mut guard = self.inner.lock().await;
        guard.set_buffer_size_fn(buffer_size, f).await?;
        guard.flush_writer().await
    }

    pub async fn meow_fn<F: FnOnce(MeowResponse) + 'static>(&self, f: F) -> Result<(), Error> {
        let mut guard = self.inner.lock().await;
        guard.meow_fn(f).await?;
        guard.flush_writer().await
    }
}
