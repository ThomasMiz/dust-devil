use std::io::{Error, ErrorKind};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite};

use super::SandstormCommandType;

/// A snapshot of metrics measured by the server about incoming connections, bytes sent, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Metrics {
    /// The amount of currently connected client connections.
    pub current_client_connections: u32,

    /// The total amount of incoming client connections received by the server since startup.
    pub historic_client_connections: u64,

    /// The total amount of bytes sent by clients to remote destinations.
    pub client_bytes_sent: u64,

    /// The total amount of bytes received by clients from remote destinations.
    pub client_bytes_received: u64,

    /// The amount of currently connected Sandstorm connections.
    pub current_sandstorm_connections: u32,

    /// The total amount of incoming Sandstorm connections received by the server since startup.
    pub historic_sandstorm_connections: u64,
}

impl ByteRead for Metrics {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
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
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.current_client_connections.write(writer).await?;
        self.historic_client_connections.write(writer).await?;
        self.client_bytes_sent.write(writer).await?;
        self.client_bytes_received.write(writer).await?;
        self.current_sandstorm_connections.write(writer).await?;
        self.historic_sandstorm_connections.write(writer).await
    }
}

/// A Sandstorm get current metrics request.
pub struct CurrentMetricsRequest;

/// A Sandstorm get current metrics response.
pub struct CurrentMetricsResponse(
    /// The metrics snapshot returned by the server, or `None` if the server refused to send back
    /// a metrics snapshot (if, for example, it is not tracking metrics).
    pub Option<Metrics>,
);

impl ByteRead for CurrentMetricsRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, Error> {
        Ok(Self)
    }
}

impl ByteWrite for CurrentMetricsRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        SandstormCommandType::RequestCurrentMetrics.write(writer).await
    }
}

impl ByteRead for CurrentMetricsResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(<Option<Metrics> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for CurrentMetricsResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::RequestCurrentMetrics, &self.0).write(writer).await
    }
}

/// A Sandstorm event stream config request, used to enable or disable event streaming.
pub struct EventStreamConfigRequest(
    /// The desired status for the event stream (`true` = enabled, `false` = disabled).
    pub bool,
);

/// A Sandstorm event stream config response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStreamConfigResponse {
    /// Indicates that the event stream is now disabled. If this is a response to a request to
    /// enable it, then it is the server refusing to enable the event stream.
    Disabled,

    /// Indicates that the event stream is now enabled, as well as a metrics snapshot at the time
    /// the events were enabled (this can be used to infer accurate real time metrics).
    Enabled(Metrics),

    /// Indicates that the event stream was already enabled.
    WasAlreadyEnabled,
}

impl ByteRead for EventStreamConfigRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(bool::read(reader).await?))
    }
}

impl ByteWrite for EventStreamConfigRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::EventStreamConfig, self.0).write(writer).await
    }
}

impl ByteRead for EventStreamConfigResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let value = u8::read(reader).await?;
        match value {
            0x00 => Ok(Self::Disabled),
            0x01 => Ok(Self::Enabled(Metrics::read(reader).await?)),
            0x02 => Ok(Self::WasAlreadyEnabled),
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid EventStreamConfigResponse type byte")),
        }
    }
}

impl ByteWrite for EventStreamConfigResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
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
