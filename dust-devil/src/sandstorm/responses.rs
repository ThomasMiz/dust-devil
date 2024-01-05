use std::io;

use dust_devil_core::sandstorm::SandstormHandshakeStatus;
use tokio::io::{AsyncWrite, AsyncWriteExt};

pub async fn send_handshake_response<W>(writer: &mut W, status: SandstormHandshakeStatus) -> Result<(), io::Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    writer.write_u8(status as u8).await
}
