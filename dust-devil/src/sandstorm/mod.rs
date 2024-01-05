use std::io::{self, ErrorKind};

use dust_devil_core::{
    sandstorm::{SandstormCommandType, SandstormHandshakeStatus},
    serialize::{ByteRead, ByteWrite, SmallWriteList},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter},
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

const SANDSTORM_READ_BUFFER_SIZE: usize = 1024;
const SANDSTORM_WRITE_BUFFER_SIZE: usize = 1024;

pub async fn handle_sandstorm(stream: TcpStream, mut context: SandstormContext, cancel_token: CancellationToken) {
    select! {
        result = handle_sandstorm_inner(stream, &mut context) => context.log_finished(result).await,
        _ = cancel_token.cancelled() => {}
    }
}

async fn handle_sandstorm_inner(mut stream: TcpStream, context: &mut SandstormContext) -> Result<(), io::Error> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::with_capacity(SANDSTORM_READ_BUFFER_SIZE, reader);

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

    let mut writer = BufWriter::with_capacity(SANDSTORM_WRITE_BUFFER_SIZE, writer);
    loop {
        match SandstormCommandType::read(&mut reader).await {
            Ok(command) => {
                run_command(command, &mut reader, &mut writer, context).await?;
                writer.flush().await?;
            }
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        };
    }

    Ok(())
}

async fn run_command<R, W>(
    command: SandstormCommandType,
    reader: &mut R,
    writer: &mut W,
    context: &mut SandstormContext,
) -> Result<(), io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    match command {
        // SandstormCommandType::Shutdown => {}
        // SandstormCommandType::LogEventConfig => {}
        // SandstormCommandType::LogEventStream => {}
        // SandstormCommandType::ListSocks5Sockets => {}
        // SandstormCommandType::AddSocks5Socket => {}
        // SandstormCommandType::RemoveSocks5Socket => {}
        // SandstormCommandType::ListSandstormSockets => {}
        // SandstormCommandType::AddSandstormSocket => {}
        // SandstormCommandType::RemoveSandstormSocket => {}
        SandstormCommandType::ListUsers => {
            let snapshot = context.get_users_snapshot();
            (SandstormCommandType::ListUsers, snapshot.as_slice()).write(writer).await?;
        }
        // SandstormCommandType::AddUser => {}
        // SandstormCommandType::UpdateUser => {}
        // SandstormCommandType::DeleteUser => {}
        SandstormCommandType::ListAuthMethods => {
            let auth_methods = context.get_auth_methods();
            (SandstormCommandType::ListAuthMethods, SmallWriteList(auth_methods.as_slice()))
                .write(writer)
                .await?;
        }
        SandstormCommandType::ToggleAuthMethod => {
            let auth_method = reader.read_u8().await?;
            let state = bool::read(reader).await?;
            let result = context.toggle_auth_method(auth_method, state);
            (SandstormCommandType::ToggleAuthMethod, result).write(writer).await?;
        }
        // SandstormCommandType::RequestCurrentMetrics => {}
        SandstormCommandType::GetBufferSize => {
            let buffer_size = context.get_buffer_size();
            (SandstormCommandType::GetBufferSize, buffer_size).write(writer).await?;
        }
        SandstormCommandType::SetBufferSize => {
            let buffer_size = reader.read_u32().await?;
            let result = context.set_buffer_size(buffer_size);
            (SandstormCommandType::SetBufferSize, result).write(writer).await?;
        }
        SandstormCommandType::Meow => {
            SandstormCommandType::Meow.write(writer).await?;
            writer.write_all(b"MEOW").await?;
        }
        c => {
            eprintln!("Yea I dunno what to do with {c:?} yet ((is not implemented 💀))");
            return Err(io::Error::new(ErrorKind::Unsupported, format!("Yea I dunno what to do with {c:?}")));
        }
    }

    Ok(())
}
