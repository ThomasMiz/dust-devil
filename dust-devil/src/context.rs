use std::{
    io::Error,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
};

use dust_devil_core::{
    logging::{Event, EventData},
    sandstorm::{AddUserResponse, DeleteUserResponse, Metrics, RemoveSocketResponse, UpdateUserResponse},
    socks5::AuthMethod,
    users::UserRole,
};
use tokio::sync::{
    broadcast,
    mpsc::Sender,
    oneshot::{self, Receiver},
};

use crate::{
    logger::{LogSender, MetricsRequester},
    messaging::MessageType,
    users::UserManager,
};

pub struct ServerState {
    users: UserManager,
    no_auth_enabled: AtomicBool,
    userpass_auth_enabled: AtomicBool,
    buffer_size: AtomicU32,
    message_sender: Sender<MessageType>,
    metrics_requester: Option<MetricsRequester>,
}

impl ServerState {
    pub fn new(
        users: UserManager,
        no_auth_enabled: bool,
        userpass_auth_enabled: bool,
        buffer_size: u32,
        message_sender: Sender<MessageType>,
        metrics_requester: Option<MetricsRequester>,
    ) -> Self {
        ServerState {
            users,
            no_auth_enabled: AtomicBool::new(no_auth_enabled),
            userpass_auth_enabled: AtomicBool::new(userpass_auth_enabled),
            buffer_size: AtomicU32::new(buffer_size),
            message_sender,
            metrics_requester,
        }
    }

    pub fn users(&self) -> &UserManager {
        &self.users
    }
}

pub struct ClientContext {
    pub client_id: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub state: Arc<ServerState>,
    pub log_sender: Option<LogSender>,
}

#[macro_export]
macro_rules! log {
    ($cx:expr, $event:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send($event);
        }
    };
}

impl ClientContext {
    pub fn create(client_id: u64, state: &Arc<ServerState>, log_sender: Option<LogSender>) -> Self {
        ClientContext {
            client_id,
            bytes_sent: 0,
            bytes_received: 0,
            state: Arc::clone(state),
            log_sender,
        }
    }

    pub fn buffer_size(&self) -> usize {
        self.state.buffer_size.load(Ordering::Relaxed) as usize
    }

    pub fn is_noauth_enabled(&self) -> bool {
        self.state.no_auth_enabled.load(Ordering::Relaxed)
    }

    pub fn is_userpass_enabled(&self) -> bool {
        self.state.userpass_auth_enabled.load(Ordering::Relaxed)
    }

    pub fn try_login(&self, username: &str, password: &str) -> bool {
        self.state.users.try_login(username, password).is_some()
    }

    pub fn register_bytes_sent(&mut self, count: u64) {
        self.bytes_sent += count;
        log!(self, EventData::ClientBytesSent(self.client_id, count));
    }

    pub fn register_bytes_received(&mut self, count: u64) {
        self.bytes_received += count;
        log!(self, EventData::ClientBytesReceived(self.client_id, count));
    }
}

#[macro_export]
macro_rules! log_socks_finished {
    ($cx:expr, $result:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientConnectionFinished(
                $cx.client_id,
                $cx.bytes_sent,
                $cx.bytes_received,
                $result,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_unsupported_version {
    ($cx:expr, $version:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientRequestedUnsupportedVersion(
                $cx.client_id,
                $version,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_unsupported_atyp {
    ($cx:expr, $atyp:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientRequestedUnsupportedAtyp(
                $cx.client_id,
                $atyp,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_unsupported_command {
    ($cx:expr, $cmd:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientRequestedUnsupportedCommand(
                $cx.client_id,
                $cmd,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_selected_auth {
    ($cx:expr, $maybe_auth_method:expr) => {
        if let Some(sender) = &$cx.log_sender {
            match $maybe_auth_method {
                Some(auth_method) => {
                    sender.send(dust_devil_core::logging::EventData::ClientSelectedAuthMethod(
                        $cx.client_id,
                        auth_method,
                    ));
                }
                None => {
                    sender.send(dust_devil_core::logging::EventData::ClientNoAcceptableAuthMethod($cx.client_id));
                }
            }
        }
    };
}

#[macro_export]
macro_rules! log_socks_unsupported_userpass_version {
    ($cx:expr, $version:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientRequestedUnsupportedUserpassVersion(
                $cx.client_id,
                $version,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_authenticated_with_userpass {
    ($cx:expr, $username:expr, $success:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientAuthenticatedWithUserpass(
                $cx.client_id,
                $username,
                $success,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_dns_lookup {
    ($cx:expr, $domainname:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientDnsLookup($cx.client_id, $domainname));
        }
    };
}

#[macro_export]
macro_rules! log_socks_connection_attempt {
    ($cx:expr, $address:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientAttemptingConnect(
                $cx.client_id,
                $address,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_connection_attempt_bind_failed {
    ($cx:expr, $error:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientConnectionAttemptBindFailed(
                $cx.client_id,
                $error,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_connection_attempt_connect_failed {
    ($cx:expr, $error:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientConnectionAttemptConnectFailed(
                $cx.client_id,
                $error,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_connect_to_destination_failed {
    ($cx:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientFailedToConnectToDestination(
                $cx.client_id,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_connected_to_destination {
    ($cx:expr, $address:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientConnectedToDestination(
                $cx.client_id,
                $address,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_socks_source_shutdown {
    ($cx:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientSourceShutdown($cx.client_id));
        }
    };
}

#[macro_export]
macro_rules! log_socks_destination_shutdown {
    ($cx:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::ClientDestinationShutdown($cx.client_id));
        }
    };
}

pub struct SandstormContext {
    pub manager_id: u64,
    pub state: Arc<ServerState>,
    pub log_sender: Option<LogSender>,
}

impl SandstormContext {
    pub fn create(manager_id: u64, state: &Arc<ServerState>, log_sender: Option<LogSender>) -> Self {
        SandstormContext {
            manager_id,
            state: Arc::clone(state),
            log_sender,
        }
    }

    pub fn try_login(&self, username: &str, password: &str) -> Option<bool> {
        self.state.users.try_login(username, password).map(|u| u == UserRole::Admin)
    }

    pub async fn request_shutdown(&self) -> Receiver<()> {
        log!(self, EventData::SandstormRequestedShutdown(self.manager_id));
        let (result_tx, result_rx) = oneshot::channel();
        let _ = self.state.message_sender.send(MessageType::ShutdownRequest(result_tx)).await;

        result_rx
    }

    pub async fn list_socks5_sockets(&self) -> Receiver<Vec<SocketAddr>> {
        let (result_tx, result_rx) = oneshot::channel();
        let _ = self.state.message_sender.send(MessageType::ListSocks5Sockets(result_tx)).await;

        result_rx
    }

    pub async fn add_socks5_socket(&self, socket_address: SocketAddr) -> Receiver<Result<(), Error>> {
        log!(self, EventData::NewSocksSocketRequestedByManager(self.manager_id, socket_address));

        let (result_tx, result_rx) = oneshot::channel();
        let _ = self
            .state
            .message_sender
            .send(MessageType::AddSocks5Socket(socket_address, result_tx))
            .await;

        result_rx
    }

    pub async fn remove_socks5_socket(&self, socket_address: SocketAddr) -> Receiver<RemoveSocketResponse> {
        log!(
            self,
            EventData::RemoveSocksSocketRequestedByManager(self.manager_id, socket_address)
        );

        let (result_tx, result_rx) = oneshot::channel();
        let _ = self
            .state
            .message_sender
            .send(MessageType::RemoveSocks5Socket(socket_address, result_tx))
            .await;

        result_rx
    }

    pub async fn list_sandstorm_sockets(&self) -> Receiver<Vec<SocketAddr>> {
        let (result_tx, result_rx) = oneshot::channel();
        let _ = self.state.message_sender.send(MessageType::ListSandstormSockets(result_tx)).await;

        result_rx
    }

    pub async fn add_sandstorm_socket(&self, socket_address: SocketAddr) -> Receiver<Result<(), Error>> {
        log!(
            self,
            EventData::NewSandstormSocketRequestedByManager(self.manager_id, socket_address)
        );

        let (result_tx, result_rx) = oneshot::channel();
        let _ = self
            .state
            .message_sender
            .send(MessageType::AddSandstormSocket(socket_address, result_tx))
            .await;

        result_rx
    }

    pub async fn remove_sandstorm_socket(&self, socket_address: SocketAddr) -> Receiver<RemoveSocketResponse> {
        log!(
            self,
            EventData::RemoveSandstormSocketRequestedByManager(self.manager_id, socket_address)
        );

        let (result_tx, result_rx) = oneshot::channel();
        let _ = self
            .state
            .message_sender
            .send(MessageType::RemoveSandstormSocket(socket_address, result_tx))
            .await;

        result_rx
    }

    pub fn get_users_snapshot(&self) -> Vec<(String, UserRole)> {
        self.state.users.take_snapshot()
    }

    pub fn add_user(&self, username: String, password: String, role: UserRole) -> AddUserResponse {
        for c in username.chars() {
            if c.is_control() || (c.is_ascii() && !c.is_ascii_graphic()) {
                return AddUserResponse::InvalidValues;
            }
        }

        if self.state.users.insert(username.clone(), password, role) {
            log!(self, EventData::UserRegisteredByManager(self.manager_id, username, role));

            AddUserResponse::Ok
        } else {
            AddUserResponse::AlreadyExists
        }
    }

    pub fn update_user(&self, username: String, password: Option<String>, role: Option<UserRole>) -> UpdateUserResponse {
        if password.is_none() && role.is_none() {
            return UpdateUserResponse::NothingWasRequested;
        }

        let has_password = password.is_some();
        match self.state.users.update(username.clone(), password, role) {
            Ok(Some(role)) => {
                log!(self, EventData::UserUpdatedByManager(self.manager_id, username, role, has_password));
                UpdateUserResponse::Ok
            }
            Ok(None) => UpdateUserResponse::CannotDeleteOnlyAdmin,
            Err(()) => UpdateUserResponse::UserNotFound,
        }
    }

    pub fn delete_user(&self, username: String) -> DeleteUserResponse {
        match self.state.users.delete(username) {
            Ok(Some((username, role))) => {
                log!(self, EventData::UserDeletedByManager(self.manager_id, username, role));
                DeleteUserResponse::Ok
            }
            Ok(None) => DeleteUserResponse::CannotDeleteOnlyAdmin,
            Err(()) => DeleteUserResponse::UserNotFound,
        }
    }

    pub fn get_auth_methods(&self) -> Vec<(AuthMethod, bool)> {
        vec![
            (AuthMethod::NoAuth, self.state.no_auth_enabled.load(Ordering::Relaxed)),
            (
                AuthMethod::UsernameAndPassword,
                self.state.userpass_auth_enabled.load(Ordering::Relaxed),
            ),
        ]
    }

    pub fn toggle_auth_method(&self, auth_method: AuthMethod, state: bool) -> bool {
        match auth_method {
            AuthMethod::NoAuth => self.state.no_auth_enabled.store(state, Ordering::Relaxed),
            AuthMethod::UsernameAndPassword => self.state.userpass_auth_enabled.store(state, Ordering::Relaxed),
            // _ => return false,
        }

        log!(self, EventData::AuthMethodToggledByManager(self.manager_id, auth_method, state));
        true
    }

    pub async fn request_metrics(&self) -> Option<Receiver<Metrics>> {
        match &self.state.metrics_requester {
            Some(requester) => requester.request_metrics().await,
            None => None,
        }
    }

    pub async fn request_metrics_and_subscribe(&self) -> Option<Receiver<(Metrics, broadcast::Receiver<Arc<Event>>)>> {
        match &self.state.metrics_requester {
            Some(requester) => requester.request_metrics_and_subscribe().await,
            None => None,
        }
    }

    pub fn get_buffer_size(&self) -> u32 {
        self.state.buffer_size.load(Ordering::Relaxed)
    }

    pub fn set_buffer_size(&self, value: u32) -> bool {
        if value == 0 {
            return false;
        }

        log!(self, EventData::BufferSizeChangedByManager(self.manager_id, value));

        self.state.buffer_size.store(value, Ordering::Relaxed);
        true
    }
}

#[macro_export]
macro_rules! log_sandstorm_finished {
    ($cx:expr, $result:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::SandstormConnectionFinished(
                $cx.manager_id,
                $result,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_sandstorm_unsupported_version {
    ($cx:expr, $version:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::SandstormRequestedUnsupportedVersion(
                $cx.manager_id,
                $version,
            ));
        }
    };
}

#[macro_export]
macro_rules! log_sandstorm_authenticated_as {
    ($cx:expr, $username:expr, $success:expr) => {
        if let Some(sender) = &$cx.log_sender {
            sender.send(dust_devil_core::logging::EventData::SandstormAuthenticatedAs(
                $cx.manager_id,
                $username,
                $success,
            ));
        }
    };
}
