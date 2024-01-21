use std::io::Error;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    logging::Event,
    serialize::{ByteRead, ByteWrite},
};

use super::SandstormCommandType;

/// A Sandstorm event stream message.
pub struct EventStreamResponse(
    /// The new event sent by the server.
    pub Event,
);

/// A borrowed version of [`EventStreamResponse`].
pub struct EventStreamResponseRef<'a>(
    /// The new event sent by the server.
    pub &'a Event,
);

impl EventStreamResponse {
    pub fn as_ref(&self) -> EventStreamResponseRef {
        EventStreamResponseRef(&self.0)
    }
}

impl ByteRead for EventStreamResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(Event::read(reader).await?))
    }
}

impl ByteWrite for EventStreamResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for EventStreamResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::EventStream, &self.0).write(writer).await
    }
}
