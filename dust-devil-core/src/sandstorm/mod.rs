use std::io::{self, ErrorKind};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    serialize::{ByteRead, ByteWrite},
    u8_repr_enum::U8ReprEnum,
};

mod auth_methods;
mod buffer_size;
mod event_stream;
mod event_stream_config;
mod handshake;
mod meow;
mod sandstorm_sockets;
mod shutdown;
mod socks5_sockets;
mod users;

pub use auth_methods::*;
pub use buffer_size::*;
pub use event_stream::*;
pub use event_stream_config::*;
pub use handshake::*;
pub use meow::*;
pub use sandstorm_sockets::*;
pub use shutdown::*;
pub use socks5_sockets::*;
pub use users::*;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandstormCommandType {
    Shutdown = 0x00,
    EventStreamConfig = 0x01,
    EventStream = 0x02,
    ListSocks5Sockets = 0x03,
    AddSocks5Socket = 0x04,
    RemoveSocks5Socket = 0x05,
    ListSandstormSockets = 0x06,
    AddSandstormSocket = 0x07,
    RemoveSandstormSocket = 0x08,
    ListUsers = 0x09,
    AddUser = 0x0A,
    UpdateUser = 0x0B,
    DeleteUser = 0x0C,
    ListAuthMethods = 0x0D,
    ToggleAuthMethod = 0x0E,
    RequestCurrentMetrics = 0x0F,
    GetBufferSize = 0x10,
    SetBufferSize = 0x11,
    Meow = 0xFF,
}

impl U8ReprEnum for SandstormCommandType {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Shutdown),
            0x01 => Some(Self::EventStreamConfig),
            0x02 => Some(Self::EventStream),
            0x03 => Some(Self::ListSocks5Sockets),
            0x04 => Some(Self::AddSocks5Socket),
            0x05 => Some(Self::RemoveSocks5Socket),
            0x06 => Some(Self::ListSandstormSockets),
            0x07 => Some(Self::AddSandstormSocket),
            0x08 => Some(Self::RemoveSandstormSocket),
            0x09 => Some(Self::ListUsers),
            0x0A => Some(Self::AddUser),
            0x0B => Some(Self::UpdateUser),
            0x0C => Some(Self::DeleteUser),
            0x0D => Some(Self::ListAuthMethods),
            0x0E => Some(Self::ToggleAuthMethod),
            0x0F => Some(Self::RequestCurrentMetrics),
            0x10 => Some(Self::GetBufferSize),
            0x11 => Some(Self::SetBufferSize),
            0xFF => Some(Self::Meow),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteWrite for SandstormCommandType {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.into_u8().write(writer).await
    }
}

impl ByteRead for SandstormCommandType {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match SandstormCommandType::from_u8(u8::read(reader).await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid SandstormCommandType type byte")),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveSocketResponse {
    Ok = 0x00,
    SocketNotFound = 0x01,
}

impl U8ReprEnum for RemoveSocketResponse {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Ok),
            0x01 => Some(Self::SocketNotFound),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteRead for RemoveSocketResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid RemoveSocketResponse type byte")),
        }
    }
}

impl ByteWrite for RemoveSocketResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.into_u8().write(writer).await
    }
}
