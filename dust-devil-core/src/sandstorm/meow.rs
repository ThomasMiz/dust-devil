use std::io::{self, ErrorKind};

use tokio::io::AsyncReadExt;

use crate::serialize::{ByteRead, ByteWrite};

use super::SandstormCommandType;

pub struct MeowRequest;
pub struct MeowResponse;

impl ByteRead for MeowRequest {
    async fn read<R: tokio::io::AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
}

impl ByteWrite for MeowRequest {
    async fn write<W: tokio::io::AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        SandstormCommandType::Meow.write(writer).await
    }
}

impl ByteRead for MeowResponse {
    async fn read<R: tokio::io::AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, std::io::Error> {
        let mut meow = [0u8; 4];
        reader.read_exact(&mut meow).await?;

        if meow == [b'M', b'E', b'O', b'W'] {
            Ok(Self)
        } else {
            Err(io::Error::new(
                ErrorKind::InvalidData,
                "Server responded to meow, but did not say MEOW!",
            ))
        }
    }
}

impl ByteWrite for MeowResponse {
    async fn write<W: tokio::io::AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        (SandstormCommandType::Meow, b'M', b'E', b'O', b'W').write(writer).await
    }
}
