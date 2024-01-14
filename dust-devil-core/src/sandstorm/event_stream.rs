use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    logging::Event,
    serialize::{ByteRead, ByteWrite},
};

use super::SandstormCommandType;

pub struct EventStreamResponse(pub Event);
pub struct EventStreamResponseRef<'a>(pub &'a Event);

impl EventStreamResponse {
    pub fn as_ref(&self) -> EventStreamResponseRef {
        EventStreamResponseRef(&self.0)
    }
}

impl ByteRead for EventStreamResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(Event::read(reader).await?))
    }
}

impl ByteWrite for EventStreamResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for EventStreamResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::EventStream, &self.0).write(writer).await
    }
}
