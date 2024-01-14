use std::{io, net::SocketAddr};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite};

use super::{RemoveSocketResponse, SandstormCommandType};

pub struct ListSandstormSocketsRequest;
pub struct ListSandstormSocketsResponse(pub Vec<SocketAddr>);
pub struct ListSandstormSocketsResponseRef<'a>(&'a [SocketAddr]);

impl ListSandstormSocketsResponse {
    pub fn as_ref(&self) -> ListSandstormSocketsResponseRef {
        ListSandstormSocketsResponseRef(&self.0)
    }
}

impl ByteRead for ListSandstormSocketsRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self)
    }
}

impl ByteWrite for ListSandstormSocketsRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        SandstormCommandType::ListSandstormSockets.write(writer).await
    }
}

impl ByteRead for ListSandstormSocketsResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(<Vec<SocketAddr> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for ListSandstormSocketsResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for ListSandstormSocketsResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::ListSandstormSockets, self.0).write(writer).await
    }
}

pub struct AddSandstormSocketRequest(pub SocketAddr);
pub struct AddSandstormSocketResponse(pub Result<(), io::Error>);

impl ByteRead for AddSandstormSocketRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(SocketAddr::read(reader).await?))
    }
}

impl ByteWrite for AddSandstormSocketRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::AddSandstormSocket, &self.0).write(writer).await
    }
}

impl ByteRead for AddSandstormSocketResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(<Result<(), io::Error> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for AddSandstormSocketResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::AddSandstormSocket, &self.0).write(writer).await
    }
}

pub struct RemoveSandstormSocketRequest(pub SocketAddr);
pub struct RemoveSandstormSocketResponse(pub RemoveSocketResponse);

impl ByteRead for RemoveSandstormSocketRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(SocketAddr::read(reader).await?))
    }
}

impl ByteWrite for RemoveSandstormSocketRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::RemoveSandstormSocket, &self.0).write(writer).await
    }
}

impl ByteRead for RemoveSandstormSocketResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(RemoveSocketResponse::read(reader).await?))
    }
}

impl ByteWrite for RemoveSandstormSocketResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::RemoveSandstormSocket, &self.0).write(writer).await
    }
}
