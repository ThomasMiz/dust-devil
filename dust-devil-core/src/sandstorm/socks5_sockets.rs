use std::{io, net::SocketAddr};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite};

use super::{RemoveSocketResponse, SandstormCommandType};

pub struct ListSocks5SocketsRequest;
pub struct ListSocks5SocketsResponse(pub Vec<SocketAddr>);
pub struct ListSocks5SocketsResponseRef<'a>(&'a [SocketAddr]);

impl ListSocks5SocketsResponse {
    pub fn as_ref(&self) -> ListSocks5SocketsResponseRef {
        ListSocks5SocketsResponseRef(&self.0)
    }
}

impl ByteRead for ListSocks5SocketsRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self)
    }
}

impl ByteWrite for ListSocks5SocketsRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        SandstormCommandType::ListSocks5Sockets.write(writer).await
    }
}

impl ByteRead for ListSocks5SocketsResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(<Vec<SocketAddr> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for ListSocks5SocketsResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for ListSocks5SocketsResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::ListSocks5Sockets, self.0).write(writer).await
    }
}

pub struct AddSocks5SocketRequest(pub SocketAddr);
pub struct AddSocks5SocketResponse(pub Result<(), io::Error>);

impl ByteRead for AddSocks5SocketRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(SocketAddr::read(reader).await?))
    }
}

impl ByteWrite for AddSocks5SocketRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::AddSocks5Socket, &self.0).write(writer).await
    }
}

impl ByteRead for AddSocks5SocketResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(<Result<(), io::Error> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for AddSocks5SocketResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::AddSocks5Socket, &self.0).write(writer).await
    }
}

pub struct RemoveSocks5SocketRequest(pub SocketAddr);
pub struct RemoveSocks5SocketResponse(pub RemoveSocketResponse);

impl ByteRead for RemoveSocks5SocketRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(SocketAddr::read(reader).await?))
    }
}

impl ByteWrite for RemoveSocks5SocketRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::RemoveSocks5Socket, &self.0).write(writer).await
    }
}

impl ByteRead for RemoveSocks5SocketResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(RemoveSocketResponse::read(reader).await?))
    }
}

impl ByteWrite for RemoveSocks5SocketResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::RemoveSocks5Socket, &self.0).write(writer).await
    }
}
