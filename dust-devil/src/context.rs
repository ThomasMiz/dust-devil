use std::{
    io,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
};

use dust_devil_core::{
    logging::LogEventType,
    sandstorm::{AddUserResponse, DeleteUserResponse, UpdateUserResponse},
    socks5::AuthMethod,
    users::UserRole,
};
use tokio::sync::mpsc::error::TrySendError;

use crate::{logger::LogSender, printlnif, users::UserManager};

pub struct ServerState {
    verbose: bool,
    users: UserManager,
    no_auth_enabled: AtomicBool,
    userpass_auth_enabled: AtomicBool,
    current_client_connections: AtomicU32,
    historic_client_connections: AtomicU64,
    client_bytes_sent: AtomicU64,
    client_bytes_received: AtomicU64,
    current_sandstorm_connections: AtomicU32,
    historic_sandstorm_connections: AtomicU64,
    buffer_size: AtomicU32,
}

impl ServerState {
    pub fn new(verbose: bool, users: UserManager, no_auth_enabled: bool, userpass_auth_enabled: bool, buffer_size: u32) -> Self {
        ServerState {
            verbose,
            users,
            no_auth_enabled: AtomicBool::new(no_auth_enabled),
            userpass_auth_enabled: AtomicBool::new(userpass_auth_enabled),
            historic_client_connections: AtomicU64::new(0),
            client_bytes_sent: AtomicU64::new(0),
            current_client_connections: AtomicU32::new(0),
            client_bytes_received: AtomicU64::new(0),
            current_sandstorm_connections: AtomicU32::new(0),
            historic_sandstorm_connections: AtomicU64::new(0),
            buffer_size: AtomicU32::new(buffer_size),
        }
    }

    pub fn users(&self) -> &UserManager {
        &self.users
    }
}

pub struct ClientContext {
    client_id: u64,
    bytes_sent: u64,
    bytes_received: u64,
    state: Arc<ServerState>,
    log_sender: LogSender,
}

impl ClientContext {
    pub fn create(client_id: u64, state: &Arc<ServerState>, log_sender: LogSender) -> Self {
        let context = ClientContext {
            client_id,
            bytes_sent: 0,
            bytes_received: 0,
            state: Arc::clone(state),
            log_sender,
        };

        context.state.current_client_connections.fetch_add(1, Ordering::Relaxed);
        context.state.historic_client_connections.fetch_add(1, Ordering::Relaxed);

        context
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
        self.state.client_bytes_sent.fetch_add(count, Ordering::Relaxed);
        let _ = self.log_sender.try_send(LogEventType::ClientBytesSent(self.client_id, count));
    }

    pub fn register_bytes_received(&mut self, count: u64) {
        self.bytes_received += count;
        self.state.client_bytes_received.fetch_add(count, Ordering::Relaxed);
        let _ = self.log_sender.try_send(LogEventType::ClientBytesReceived(self.client_id, count));
    }
}

impl ClientContext {
    pub async fn log_finished(&self, result: Result<(), io::Error>) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientConnectionFinished(
                self.client_id,
                self.bytes_sent,
                self.bytes_received,
                result,
            ))
            .await;
    }

    pub async fn log_unsupported_socks_version(&self, version: u8) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientRequestedUnsupportedVersion(self.client_id, version))
            .await;
    }

    pub async fn log_unsupported_atyp(&self, atyp: u8) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientRequestedUnsupportedAtyp(self.client_id, atyp))
            .await;
    }

    pub async fn log_unsupported_socks_command(&self, cmd: u8) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientRequestedUnsupportedCommand(self.client_id, cmd))
            .await;
    }

    pub async fn log_selected_auth(&self, auth_method: AuthMethod) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientSelectedAuthMethod(self.client_id, auth_method))
            .await;
    }

    pub async fn log_unsupported_userpass_version(&self, version: u8) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientRequestedUnsupportedUserpassVersion(self.client_id, version))
            .await;
    }

    pub async fn log_authenticated_with_userpass(&self, username: String, success: bool) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientAuthenticatedWithUserpass(self.client_id, username, success))
            .await;
    }

    pub async fn log_dns_lookup(&self, domainname: String) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientDnsLookup(self.client_id, domainname))
            .await;
    }

    pub async fn log_connection_attempt(&self, address: SocketAddr) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientAttemptingConnect(self.client_id, address))
            .await;
    }

    pub async fn log_connection_attempt_bind_failed(&self, error: io::Error) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientConnectionAttemptBindFailed(self.client_id, error))
            .await;
    }

    pub async fn log_connection_attempt_connect_failed(&self, error: io::Error) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientConnectionAttemptConnectFailed(self.client_id, error))
            .await;
    }

    pub async fn log_connect_to_destination_failed(&self) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientFailedToConnectToDestination(self.client_id))
            .await;
    }

    pub async fn log_connected_to_destination(&self, address: SocketAddr) {
        let _ = self
            .log_sender
            .send(LogEventType::ClientConnectedToDestination(self.client_id, address))
            .await;
    }

    pub fn log_source_shutdown(&self) {
        if let Err(TrySendError::Full(e)) = self.log_sender.try_send(LogEventType::ClientSourceShutdown(self.client_id)) {
            printlnif!(self.state.verbose, "Verbose warning, log event lost: {}", e.data);
        }
    }

    pub fn log_destination_shutdown(&self) {
        if let Err(TrySendError::Full(e)) = self.log_sender.try_send(LogEventType::ClientDestinationShutdown(self.client_id)) {
            printlnif!(self.state.verbose, "Verbose warning, log event lost: {}", e.data);
        }
    }
}

impl Drop for ClientContext {
    fn drop(&mut self) {
        self.state.current_client_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

pub struct SandstormContext {
    manager_id: u64,
    state: Arc<ServerState>,
    log_sender: LogSender,
}

impl SandstormContext {
    pub fn create(manager_id: u64, state: &Arc<ServerState>, log_sender: LogSender) -> Self {
        let context = SandstormContext {
            manager_id,
            state: Arc::clone(state),
            log_sender,
        };

        context.state.current_sandstorm_connections.fetch_add(1, Ordering::Relaxed);
        context.state.historic_sandstorm_connections.fetch_add(1, Ordering::Relaxed);

        context
    }

    pub fn try_login(&self, username: &str, password: &str) -> Option<bool> {
        self.state.users.try_login(username, password).map(|u| u == UserRole::Admin)
    }

    pub fn get_users_snapshot(&self) -> Vec<(String, UserRole)> {
        self.state.users.take_snapshot()
    }

    pub async fn add_user(&self, username: String, password: String, role: u8) -> AddUserResponse {
        let role = match UserRole::from_u8(role) {
            Some(r) => r,
            None => return AddUserResponse::InvalidValues,
        };

        if self.state.users.insert(username.clone(), password, role) {
            let _ = self
                .log_sender
                .send(LogEventType::UserRegisteredByManager(self.manager_id, username, role))
                .await;

            AddUserResponse::Ok
        } else {
            AddUserResponse::AlreadyExists
        }
    }

    pub async fn update_user(&self, username: String, password: Option<String>, role: Option<UserRole>) -> UpdateUserResponse {
        if password.is_none() && role.is_none() {
            return UpdateUserResponse::NothingWasRequested;
        }

        let has_password = password.is_some();
        match self.state.users.update(username.clone(), password, role) {
            Ok(Some(role)) => {
                let _ = self
                    .log_sender
                    .send(LogEventType::UserUpdatedByManager(self.manager_id, username, role, has_password))
                    .await;
                UpdateUserResponse::Ok
            }
            Ok(None) => UpdateUserResponse::CannotRemoveOnlyAdmin,
            Err(()) => UpdateUserResponse::UserNotFound,
        }
    }

    pub async fn delete_user(&self, username: String) -> DeleteUserResponse {
        match self.state.users.delete(username) {
            Ok(Some((username, role))) => {
                let _ = self
                    .log_sender
                    .send(LogEventType::UserDeletedByManager(self.manager_id, username, role))
                    .await;
                DeleteUserResponse::Ok
            }
            Ok(None) => DeleteUserResponse::CannotRemoveOnlyAdmin,
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

    pub async fn toggle_auth_method(&self, auth_method: u8, state: bool) -> bool {
        let auth_method = match AuthMethod::from_u8(auth_method) {
            Some(a) => a,
            None => return false,
        };

        match auth_method {
            AuthMethod::NoAuth => self.state.no_auth_enabled.store(state, Ordering::Relaxed),
            AuthMethod::UsernameAndPassword => self.state.userpass_auth_enabled.store(state, Ordering::Relaxed),
            _ => return false,
        }

        let _ = self
            .log_sender
            .send(LogEventType::AuthMethodToggledByManager(self.manager_id, auth_method, state))
            .await;
        true
    }

    pub fn get_buffer_size(&self) -> u32 {
        self.state.buffer_size.load(Ordering::Relaxed)
    }

    pub async fn set_buffer_size(&self, value: u32) -> bool {
        if value == 0 {
            return false;
        }

        let _ = self
            .log_sender
            .send(LogEventType::BufferSizeChangedByManager(self.manager_id, value))
            .await;

        self.state.buffer_size.store(value, Ordering::Relaxed);
        true
    }
}

impl SandstormContext {
    pub async fn log_finished(&self, result: Result<(), io::Error>) {
        let _ = self
            .log_sender
            .send(LogEventType::SandstormConnectionFinished(self.manager_id, result))
            .await;
    }

    pub async fn log_unsupported_sandstorm_version(&self, version: u8) {
        let _ = self
            .log_sender
            .send(LogEventType::SandstormRequestedUnsupportedVersion(self.manager_id, version))
            .await;
    }

    pub async fn log_authenticated_as(&self, username: String, success: bool) {
        let _ = self
            .log_sender
            .send(LogEventType::SandstormAuthenticatedAs(self.manager_id, username, success))
            .await;
    }
}

impl Drop for SandstormContext {
    fn drop(&mut self) {
        self.state.current_sandstorm_connections.fetch_sub(1, Ordering::Relaxed);
    }
}
