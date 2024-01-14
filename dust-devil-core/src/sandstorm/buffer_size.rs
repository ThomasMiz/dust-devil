use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite};

use super::SandstormCommandType;

pub struct GetBufferSizeRequest;
pub struct GetBufferSizeResponse(pub u32);

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

pub struct SetBufferSizeRequest(pub u32);
pub struct SetBufferSizeResponse(pub bool);

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
