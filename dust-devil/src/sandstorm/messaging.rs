use std::{io::Error, net::SocketAddr};

use dust_devil_core::{
    sandstorm::{AddUserResponse, DeleteUserResponse, Metrics, RemoveSocketResponse, UpdateUserResponse},
    socks5::AuthMethod,
    users::UserRole,
};
use tokio::sync::oneshot::Receiver;

pub enum ResponseNotification {
    Shutdown(Receiver<()>),
    LogEventConfig(bool),
    ListSocks5Sockets(Receiver<Vec<SocketAddr>>),
    AddSocks5Socket(Receiver<Result<(), Error>>),
    RemoveSocks5Socket(Receiver<RemoveSocketResponse>),
    ListSandstormSockets(Receiver<Vec<SocketAddr>>),
    AddSandstormSocket(Receiver<Result<(), Error>>),
    RemoveSandstormSocket(Receiver<RemoveSocketResponse>),
    ListUsers(Vec<(String, UserRole)>),
    AddUser(AddUserResponse),
    UpdateUser(UpdateUserResponse),
    DeleteUser(DeleteUserResponse),
    ListAuthMethods(Vec<(AuthMethod, bool)>),
    ToggleAuthMethod(bool),
    RequestCurrentMetrics(Option<Receiver<Metrics>>),
    GetBufferSize(u32),
    SetBufferSize(bool),
    Meow,
}
