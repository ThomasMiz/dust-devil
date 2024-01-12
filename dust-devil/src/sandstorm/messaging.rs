use std::{io, net::SocketAddr};

use dust_devil_core::{
    sandstorm::{AddUserResponse, DeleteUserResponse, UpdateUserResponse},
    socks5::AuthMethod,
    users::UserRole,
};
use tokio::sync::oneshot::Receiver;

pub enum ResponseNotification {
    Shutdown(Receiver<()>),
    // LogEventConfig(),
    // LogEventStream(),
    ListSocks5Sockets(Receiver<Vec<SocketAddr>>),
    AddSocks5Socket(Receiver<Result<(), io::Error>>),
    RemoveSocks5Socket(Receiver<bool>),
    ListSandstormSockets(Receiver<Vec<SocketAddr>>),
    AddSandstormSocket(Receiver<Result<(), io::Error>>),
    RemoveSandstormSocket(Receiver<bool>),
    ListUsers(Vec<(String, UserRole)>),
    AddUser(AddUserResponse),
    UpdateUser(UpdateUserResponse),
    DeleteUser(DeleteUserResponse),
    ListAuthMethods(Vec<(AuthMethod, bool)>),
    ToggleAuthMethod(bool),
    // RequestCurrentMetrics(),
    GetBufferSize(u32),
    SetBufferSize(bool),
    Meow,
}
