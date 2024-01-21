use std::{io::Error, net::SocketAddr};

use dust_devil_core::sandstorm::RemoveSocketResponse;
use tokio::sync::oneshot::Sender;

pub enum MessageType {
    ShutdownRequest(Sender<()>),
    ListSocks5Sockets(Sender<Vec<SocketAddr>>),
    AddSocks5Socket(SocketAddr, Sender<Result<(), Error>>),
    RemoveSocks5Socket(SocketAddr, Sender<RemoveSocketResponse>),
    ListSandstormSockets(Sender<Vec<SocketAddr>>),
    AddSandstormSocket(SocketAddr, Sender<Result<(), Error>>),
    RemoveSandstormSocket(SocketAddr, Sender<RemoveSocketResponse>),
}
