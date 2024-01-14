use std::io::{self, ErrorKind};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite};

use super::SandstormCommandType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    pub current_client_connections: u32,
    pub historic_client_connections: u64,
    pub client_bytes_sent: u64,
    pub client_bytes_received: u64,
    pub current_sandstorm_connections: u32,
    pub historic_sandstorm_connections: u64,
}

impl ByteRead for Metrics {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Metrics {
            current_client_connections: u32::read(reader).await?,
            historic_client_connections: u64::read(reader).await?,
            client_bytes_sent: u64::read(reader).await?,
            client_bytes_received: u64::read(reader).await?,
            current_sandstorm_connections: u32::read(reader).await?,
            historic_sandstorm_connections: u64::read(reader).await?,
        })
    }
}

impl ByteWrite for Metrics {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.current_client_connections.write(writer).await?;
        self.historic_client_connections.write(writer).await?;
        self.client_bytes_sent.write(writer).await?;
        self.client_bytes_received.write(writer).await?;
        self.current_sandstorm_connections.write(writer).await?;
        self.historic_sandstorm_connections.write(writer).await
    }
}

pub struct CurrentMetricsRequest;
pub struct CurrentMetricsResponse(pub Option<Metrics>);

impl ByteRead for CurrentMetricsRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self)
    }
}

impl ByteWrite for CurrentMetricsRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        SandstormCommandType::RequestCurrentMetrics.write(writer).await
    }
}

impl ByteRead for CurrentMetricsResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(<Option<Metrics> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for CurrentMetricsResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::RequestCurrentMetrics, &self.0).write(writer).await
    }
}

pub struct EventStreamConfigRequest(pub bool);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStreamConfigResponse {
    Disabled,
    Enabled(Metrics),
    WasAlreadyEnabled,
}

impl ByteRead for EventStreamConfigRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(bool::read(reader).await?))
    }
}

impl ByteWrite for EventStreamConfigRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::EventStreamConfig, self.0).write(writer).await
    }
}

impl ByteRead for EventStreamConfigResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let value = u8::read(reader).await?;
        match value {
            0x00 => Ok(Self::Disabled),
            0x01 => Ok(Self::Enabled(Metrics::read(reader).await?)),
            0x02 => Ok(Self::WasAlreadyEnabled),
            _ => Err(io::Error::new(
                ErrorKind::InvalidData,
                "Invalid EventStreamConfigResponse type byte",
            )),
        }
    }
}

impl ByteWrite for EventStreamConfigResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        SandstormCommandType::EventStreamConfig.write(writer).await?;

        let type_byte: u8 = match self {
            Self::Disabled => 0x00,
            Self::Enabled(_) => 0x01,
            Self::WasAlreadyEnabled => 0x02,
        };
        type_byte.write(writer).await?;

        if let Self::Enabled(metrics) = self {
            metrics.write(writer).await?;
        }

        Ok(())
    }
}
