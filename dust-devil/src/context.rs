use std::{
    io,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
};

use dust_devil_core::{logging::LogEvent, socks5::AuthMethod};
use tokio::sync::mpsc::Sender;

use crate::users::UserManager;

pub struct ServerState {
    users: UserManager,
    no_auth_enabled: AtomicBool,
    userpass_auth_enabled: AtomicBool,
    current_client_connections: AtomicU32,
    historic_client_connections: AtomicU64,
    client_bytes_sent: AtomicU64,
    client_bytes_received: AtomicU64,
}

impl ServerState {
    pub fn new(users: UserManager, no_auth_enabled: bool, userpass_auth_enabled: bool) -> Self {
        ServerState {
            users,
            no_auth_enabled: AtomicBool::new(no_auth_enabled),
            userpass_auth_enabled: AtomicBool::new(userpass_auth_enabled),
            historic_client_connections: AtomicU64::new(0),
            client_bytes_sent: AtomicU64::new(0),
            current_client_connections: AtomicU32::new(0),
            client_bytes_received: AtomicU64::new(0),
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
    log_sender: Sender<LogEvent>,
}

impl ClientContext {
    pub fn create(client_id: u64, state: &Arc<ServerState>, log_sender: &Sender<LogEvent>) -> Self {
        let context = ClientContext {
            client_id,
            bytes_sent: 0,
            bytes_received: 0,
            state: Arc::clone(state),
            log_sender: log_sender.clone(),
        };

        context.state.current_client_connections.fetch_add(1, Ordering::Relaxed);
        context.state.historic_client_connections.fetch_add(1, Ordering::Relaxed);

        context
    }

    pub fn client_id(&self) -> u64 {
        self.client_id
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
        let _ = self.log_sender.try_send(LogEvent::ClientBytesSent(self.client_id, count));
    }

    pub fn register_bytes_received(&mut self, count: u64) {
        self.bytes_received += count;
        self.state.client_bytes_received.fetch_add(count, Ordering::Relaxed);
        let _ = self.log_sender.try_send(LogEvent::ClientBytesReceived(self.client_id, count));
    }
}

impl ClientContext {
    pub async fn log_finished(&self, result: Result<(), io::Error>) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientConnectionFinished(
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
            .send(LogEvent::ClientRequestedUnsupportedVersion(self.client_id, version))
            .await;
    }

    pub async fn log_unsupported_atyp(&self, atyp: u8) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientRequestedUnsupportedAtyp(self.client_id, atyp))
            .await;
    }

    pub async fn log_unsupported_socks_command(&self, cmd: u8) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientRequestedUnsupportedCommand(self.client_id, cmd))
            .await;
    }

    pub async fn log_selected_auth(&self, auth_method: AuthMethod) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientSelectedAuthMethod(self.client_id, auth_method))
            .await;
    }

    pub async fn log_unsupported_userpass_version(&self, version: u8) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientRequestedUnsupportedUserpassVersion(self.client_id, version))
            .await;
    }

    pub async fn log_authenticated_with_userpass(&self, username: String, success: bool) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientAuthenticatedWithUserpass(self.client_id, username, success))
            .await;
    }

    pub async fn log_dns_lookup(&self, domainname: String) {
        let _ = self.log_sender.send(LogEvent::ClientDnsLookup(self.client_id, domainname)).await;
    }

    pub async fn log_connection_attempt(&self, address: SocketAddr) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientAttemptingConnect(self.client_id, address))
            .await;
    }

    pub async fn log_connection_attempt_bind_failed(&self, error: io::Error) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientConnectionAttemptBindFailed(self.client_id, error))
            .await;
    }

    pub async fn log_connection_attempt_connect_failed(&self, error: io::Error) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientConnectionAttemptConnectFailed(self.client_id, error))
            .await;
    }

    pub async fn log_connect_to_destination_failed(&self) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientFailedToConnectToDestination(self.client_id))
            .await;
    }

    pub async fn log_connected_to_destination(&self, address: SocketAddr) {
        let _ = self
            .log_sender
            .send(LogEvent::ClientConnectedToDestination(self.client_id, address))
            .await;
    }

    pub fn log_source_shutdown(&self) {
        let _ = self.log_sender.try_send(LogEvent::ClientSourceShutdown(self.client_id));
    }

    pub fn log_destination_shutdown(&self) {
        let _ = self.log_sender.try_send(LogEvent::ClientDestinationShutdown(self.client_id));
    }
}

impl Drop for ClientContext {
    fn drop(&mut self) {
        self.state.current_client_connections.fetch_sub(1, Ordering::Relaxed);
    }
}
