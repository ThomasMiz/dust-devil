use std::{io::Error, net::SocketAddr};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite};

use super::{RemoveSocketResponse, SandstormCommandType};

/// A Sandstorm list-socks5-sockets request.
pub struct ListSocks5SocketsRequest;

/// A Sandstorm list-socks5-sockets response.
pub struct ListSocks5SocketsResponse(
    /// The list of sockets listening for incoming socks5 connections sent by the server.
    pub Vec<SocketAddr>,
);

/// A borrowed version of [`ListSocks5SocketsResponse`].
pub struct ListSocks5SocketsResponseRef<'a>(
    /// The list of sockets listening for incoming socks5 connections sent by the server.
    &'a [SocketAddr],
);

impl ListSocks5SocketsResponse {
    pub fn as_ref(&self) -> ListSocks5SocketsResponseRef {
        ListSocks5SocketsResponseRef(&self.0)
    }
}

impl ByteRead for ListSocks5SocketsRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, Error> {
        Ok(Self)
    }
}

impl ByteWrite for ListSocks5SocketsRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        SandstormCommandType::ListSocks5Sockets.write(writer).await
    }
}

impl ByteRead for ListSocks5SocketsResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(<Vec<SocketAddr> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for ListSocks5SocketsResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for ListSocks5SocketsResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::ListSocks5Sockets, self.0).write(writer).await
    }
}

/// A Sandstorm add-socks5-socket request.
pub struct AddSocks5SocketRequest(
    /// The address of the new socket to open.
    pub SocketAddr,
);

/// A Sandstorm add-socks5-socket response.
pub struct AddSocks5SocketResponse(
    /// The result of the add socket operation.
    pub Result<(), Error>,
);

/// A borrowed version of [`AddSocks5SocketResponse`].
pub struct AddSocks5SocketResponseRef<'a>(
    /// The result of the add socket operation.
    pub Result<(), &'a Error>,
);

impl AddSocks5SocketResponse {
    pub fn as_ref(&self) -> AddSocks5SocketResponseRef {
        AddSocks5SocketResponseRef(self.0.as_ref().map(|_| ()))
    }
}

impl ByteRead for AddSocks5SocketRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(SocketAddr::read(reader).await?))
    }
}

impl ByteWrite for AddSocks5SocketRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::AddSocks5Socket, &self.0).write(writer).await
    }
}

impl ByteRead for AddSocks5SocketResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(<Result<(), Error> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for AddSocks5SocketResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for AddSocks5SocketResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::AddSocks5Socket, self.0).write(writer).await
    }
}

/// A Sandstorm remove-socks5-socket request.
pub struct RemoveSocks5SocketRequest(
    /// The address of the socket to remove.
    pub SocketAddr,
);

/// A Sandstorm remove-socks5-socket response.
pub struct RemoveSocks5SocketResponse(
    /// The status of the remove socket operation.
    pub RemoveSocketResponse,
);

impl ByteRead for RemoveSocks5SocketRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(SocketAddr::read(reader).await?))
    }
}

impl ByteWrite for RemoveSocks5SocketRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::RemoveSocks5Socket, &self.0).write(writer).await
    }
}

impl ByteRead for RemoveSocks5SocketResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(RemoveSocketResponse::read(reader).await?))
    }
}

impl ByteWrite for RemoveSocks5SocketResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::RemoveSocks5Socket, &self.0).write(writer).await
    }
}
