use std::{io::Error, net::SocketAddr};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite};

use super::{RemoveSocketResponse, SandstormCommandType};

/// A Sandstorm list-sandstorm-sockets request.
pub struct ListSandstormSocketsRequest;

/// A Sandstorm list-sandstorm-sockets response.
pub struct ListSandstormSocketsResponse(
    /// The list of sockets listening for incoming Sandstorm connections sent by the server.
    pub Vec<SocketAddr>,
);

/// A borrowed version of [`ListSandstormSocketsResponse`].
pub struct ListSandstormSocketsResponseRef<'a>(
    /// The list of sockets listening for incoming Sandstorm connections sent by the server.
    &'a [SocketAddr],
);

impl ListSandstormSocketsResponse {
    pub fn as_ref(&self) -> ListSandstormSocketsResponseRef {
        ListSandstormSocketsResponseRef(&self.0)
    }
}

impl ByteRead for ListSandstormSocketsRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, Error> {
        Ok(Self)
    }
}

impl ByteWrite for ListSandstormSocketsRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        SandstormCommandType::ListSandstormSockets.write(writer).await
    }
}

impl ByteRead for ListSandstormSocketsResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(<Vec<SocketAddr> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for ListSandstormSocketsResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for ListSandstormSocketsResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::ListSandstormSockets, self.0).write(writer).await
    }
}

/// A Sandstorm add-sandstorm-socket request.
pub struct AddSandstormSocketRequest(
    /// The address of the new socket to open.
    pub SocketAddr,
);

/// A Sandstorm add-sandstorm-socket response.
pub struct AddSandstormSocketResponse(
    /// The result of the add socket operation.
    pub Result<(), Error>,
);

/// A borrowed version of [`AddSandstormSocketResponse`].
pub struct AddSandstormSocketResponseRef<'a>(
    /// The result of the add socket operation.
    pub Result<(), &'a Error>,
);

impl AddSandstormSocketResponse {
    pub fn as_ref(&self) -> AddSandstormSocketResponseRef {
        AddSandstormSocketResponseRef(self.0.as_ref().map(|_| ()))
    }
}

impl ByteRead for AddSandstormSocketRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(SocketAddr::read(reader).await?))
    }
}

impl ByteWrite for AddSandstormSocketRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::AddSandstormSocket, &self.0).write(writer).await
    }
}

impl ByteRead for AddSandstormSocketResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(<Result<(), Error> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for AddSandstormSocketResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for AddSandstormSocketResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::AddSandstormSocket, self.0).write(writer).await
    }
}

/// A Sandstorm remove-sandstorm-socket request.
pub struct RemoveSandstormSocketRequest(
    /// The address of the socket to remove.
    pub SocketAddr,
);

/// A Sandstorm remove-sandstorm-socket response.
pub struct RemoveSandstormSocketResponse(
    /// The status of the remove socket operation.
    pub RemoveSocketResponse,
);

impl ByteRead for RemoveSandstormSocketRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(SocketAddr::read(reader).await?))
    }
}

impl ByteWrite for RemoveSandstormSocketRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::RemoveSandstormSocket, &self.0).write(writer).await
    }
}

impl ByteRead for RemoveSandstormSocketResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(RemoveSocketResponse::read(reader).await?))
    }
}

impl ByteWrite for RemoveSandstormSocketResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::RemoveSandstormSocket, &self.0).write(writer).await
    }
}
