use std::io::{self, ErrorKind};

use dust_devil_core::{
    sandstorm::{SandstormCommandType, SandstormHandshakeStatus},
    serialize::{ByteRead, ByteWrite},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpStream,
    select,
};
use tokio_util::sync::CancellationToken;

use crate::context::SandstormContext;

use self::{
    parsers::{parse_handshake, ParseHandshakeError},
    responses::send_handshake_response,
};

mod parsers;
mod responses;

const SANDSTORM_BUFFER_SIZE: usize = 1024;

pub async fn handle_sandstorm(stream: TcpStream, mut context: SandstormContext, cancel_token: CancellationToken) {
    select! {
        result = handle_sandstorm_inner(stream, &mut context) => context.log_finished(result).await,
        _ = cancel_token.cancelled() => {}
    }
}

async fn handle_sandstorm_inner(mut stream: TcpStream, context: &mut SandstormContext) -> Result<(), io::Error> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::with_capacity(SANDSTORM_BUFFER_SIZE, reader);

    let handshake = match parse_handshake(&mut reader).await {
        Ok(handshake) => handshake,
        Err(ParseHandshakeError::IO(error)) => return Err(error),
        Err(parsers::ParseHandshakeError::InvalidVersion(ver)) => {
            context.log_unsupported_sandstorm_version(ver).await;
            send_handshake_response(&mut writer, SandstormHandshakeStatus::UnsupportedVersion).await?;
            return Ok(());
        }
    };

    let success = context.try_login(&handshake.username, &handshake.password);
    context.log_authenticated_as(handshake.username, success == Some(true)).await;

    send_handshake_response(
        &mut writer,
        match success {
            Some(true) => SandstormHandshakeStatus::Ok,
            Some(false) => SandstormHandshakeStatus::PermissionDenied,
            None => SandstormHandshakeStatus::InvalidUsernameOrPassword,
        },
    )
    .await?;

    loop {
        match SandstormCommandType::read(&mut reader).await {
            Ok(command) => run_command(command, &mut reader, &mut writer, context).await?,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        };
    }

    Ok(())
}

async fn run_command<R, W>(
    command: SandstormCommandType,
    _reader: &mut R,
    writer: &mut W,
    _context: &mut SandstormContext,
) -> Result<(), io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    match command {
        SandstormCommandType::Meow => {
            SandstormCommandType::Meow.write(writer).await?;
            writer.write_all(b"MEOW").await?;
        }
        c => {
            eprintln!("Yea I dunno what to do with {c:?}");
            return Err(io::Error::new(ErrorKind::Unsupported, format!("Yea I dunno what to do with {c:?}")));
        }
    }

    Ok(())
}
