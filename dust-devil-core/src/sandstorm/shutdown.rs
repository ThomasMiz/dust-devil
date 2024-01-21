use std::io::Error;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite};

use super::SandstormCommandType;

/// A Sandstorm shutdown request.
pub struct ShutdownRequest;

/// A Sandstorm shutdown response.
pub struct ShutdownResponse;

impl ByteRead for ShutdownRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, Error> {
        Ok(Self)
    }
}

impl ByteWrite for ShutdownRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        SandstormCommandType::Shutdown.write(writer).await
    }
}

impl ByteRead for ShutdownResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, Error> {
        Ok(Self)
    }
}

impl ByteWrite for ShutdownResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        SandstormCommandType::Shutdown.write(writer).await
    }
}
