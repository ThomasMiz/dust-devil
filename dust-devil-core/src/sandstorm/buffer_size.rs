use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite};

use super::SandstormCommandType;

/// A Sandstorm get buffer size request.
pub struct GetBufferSizeRequest;

// A Sandstorm get buffer size response.
pub struct GetBufferSizeResponse(
    /// The buffer size returned by the server.
    pub u32,
);

impl ByteRead for GetBufferSizeRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self)
    }
}

impl ByteWrite for GetBufferSizeRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        SandstormCommandType::GetBufferSize.write(writer).await
    }
}

impl ByteRead for GetBufferSizeResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(u32::read(reader).await?))
    }
}

impl ByteWrite for GetBufferSizeResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::GetBufferSize, self.0).write(writer).await
    }
}

/// A Sandstorm set buffer size request.
pub struct SetBufferSizeRequest(
    /// The new requested buffer size, in bytes.
    pub u32,
);

/// A Sandstorm set buffer size response.
pub struct SetBufferSizeResponse(
    /// Whether the operation succeeded.
    pub bool,
);

impl ByteRead for SetBufferSizeRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(u32::read(reader).await?))
    }
}

impl ByteWrite for SetBufferSizeRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::SetBufferSize, self.0).write(writer).await
    }
}

impl ByteRead for SetBufferSizeResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(bool::read(reader).await?))
    }
}

impl ByteWrite for SetBufferSizeResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::SetBufferSize, self.0).write(writer).await
    }
}
