use std::io;

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::utils::chunk_reader::read_chunked_utf8_string;

pub struct SandstormHandshake {
    pub username: String,
    pub password: String,
}

pub enum ParseHandshakeError {
    InvalidVersion(u8),
    IO(io::Error),
}

impl From<io::Error> for ParseHandshakeError {
    fn from(value: io::Error) -> Self {
        ParseHandshakeError::IO(value)
    }
}

pub async fn parse_handshake<R>(reader: &mut R) -> Result<SandstormHandshake, ParseHandshakeError>
where
    R: AsyncRead + Unpin + ?Sized,
{
    let ver = reader.read_u8().await?;
    if ver != 1 {
        return Err(ParseHandshakeError::InvalidVersion(ver));
    }

    let username = read_chunked_utf8_string(reader).await?;
    let password = read_chunked_utf8_string(reader).await?;

    Ok(SandstormHandshake { username, password })
}
