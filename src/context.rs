use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc,
};

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
    state: Arc<ServerState>,
}

impl ClientContext {
    pub fn create(client_id: u64, state: &Arc<ServerState>) -> Self {
        let context = ClientContext {
            client_id,
            state: Arc::clone(state),
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

    pub fn register_bytes_sent(&self, count: u64) {
        self.state.client_bytes_sent.fetch_add(count, Ordering::Relaxed);
    }

    pub fn register_bytes_received(&self, count: u64) {
        self.state.client_bytes_received.fetch_add(count, Ordering::Relaxed);
    }
}

impl Drop for ClientContext {
    fn drop(&mut self) {
        self.state.current_client_connections.fetch_sub(1, Ordering::Release);
    }
}
