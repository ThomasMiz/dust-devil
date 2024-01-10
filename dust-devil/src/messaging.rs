use std::{io, net::SocketAddr};

use tokio::sync::oneshot::Sender;

pub enum MessageType {
    ShutdownRequest(Sender<()>),
    ListSocks5Sockets(Sender<Vec<SocketAddr>>),
    AddSocks5Socket(SocketAddr, Sender<Result<(), io::Error>>),
    RemoveSocks5Socket(SocketAddr, Sender<bool>),
    ListSandstormSockets(Sender<Vec<SocketAddr>>),
    AddSandstormSocket(SocketAddr, Sender<Result<(), io::Error>>),
    RemoveSandstormSocket(SocketAddr, Sender<bool>),
}
