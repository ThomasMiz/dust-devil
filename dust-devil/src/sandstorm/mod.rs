use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
};

use dust_devil_core::{
    sandstorm::{SandstormCommandType, SandstormHandshakeStatus},
    serialize::{ByteRead, ByteWrite, SmallReadString, SmallWriteList},
    users::UserRole,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
    select,
};
use tokio_util::sync::CancellationToken;

use crate::{context::SandstormContext, log_sandstorm_authenticated_as, log_sandstorm_finished, log_sandstorm_unsupported_version};

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
        result = handle_sandstorm_inner(stream, &mut context) => log_sandstorm_finished!(context, result),
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
            log_sandstorm_unsupported_version!(context, ver);
            send_handshake_response(&mut writer, SandstormHandshakeStatus::UnsupportedVersion).await?;
            let _ = writer.shutdown().await;
            return Ok(());
        }
    };

    let success = context.try_login(&handshake.username, &handshake.password);
    log_sandstorm_authenticated_as!(context, handshake.username, success == Some(true));

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
        SandstormCommandType::Shutdown => {
            context.request_shutdown().await;
        }
        // SandstormCommandType::LogEventConfig => {}
        // SandstormCommandType::LogEventStream => {}
        SandstormCommandType::ListSocks5Sockets => {
            let sockets = context.list_socks5_sockets().await.map_err(|_| io::Error::from(ErrorKind::Other))?;
            (SandstormCommandType::ListSocks5Sockets, sockets.as_slice()).write(writer).await?;
        }
        SandstormCommandType::AddSocks5Socket => {
            let socket_address = SocketAddr::read(reader).await?;
            let result = context
                .add_socks5_socket(socket_address)
                .await
                .map_err(|_| io::Error::from(ErrorKind::Other))?;
            (SandstormCommandType::AddSocks5Socket, result).write(writer).await?;
        }
        SandstormCommandType::RemoveSocks5Socket => {
            let socket_address = SocketAddr::read(reader).await?;
            let result = context
                .remove_socks5_socket(socket_address)
                .await
                .map_err(|_| io::Error::from(ErrorKind::Other))?;
            (SandstormCommandType::RemoveSocks5Socket, !result).write(writer).await?;
        }
        SandstormCommandType::ListSandstormSockets => {
            let sockets = context
                .list_sandstorm_sockets()
                .await
                .map_err(|_| io::Error::from(ErrorKind::Other))?;
            (SandstormCommandType::ListSandstormSockets, sockets.as_slice())
                .write(writer)
                .await?;
        }
        SandstormCommandType::AddSandstormSocket => {
            let socket_address = SocketAddr::read(reader).await?;
            let result = context
                .add_sandstorm_socket(socket_address)
                .await
                .map_err(|_| io::Error::from(ErrorKind::Other))?;
            (SandstormCommandType::AddSandstormSocket, result).write(writer).await?;
        }
        SandstormCommandType::RemoveSandstormSocket => {
            let socket_address = SocketAddr::read(reader).await?;
            let result = context
                .remove_sandstorm_socket(socket_address)
                .await
                .map_err(|_| io::Error::from(ErrorKind::Other))?;
            (SandstormCommandType::RemoveSandstormSocket, !result).write(writer).await?;
        }
        SandstormCommandType::ListUsers => {
            let snapshot = context.get_users_snapshot();
            (SandstormCommandType::ListUsers, snapshot.as_slice()).write(writer).await?;
        }
        SandstormCommandType::AddUser => {
            let username = SmallReadString::read(reader).await?.0;
            let password = SmallReadString::read(reader).await?.0;
            let role = reader.read_u8().await?;

            let result = context.add_user(username, password, role).await;
            (SandstormCommandType::AddUser, result).write(writer).await?;
        }
        SandstormCommandType::UpdateUser => {
            let username = SmallReadString::read(reader).await?.0;
            let password = <Option<SmallReadString> as ByteRead>::read(reader).await?;
            let password = password.map(|s| s.0);
            let role = <Option<UserRole> as ByteRead>::read(reader).await?;

            let result = context.update_user(username, password, role).await;
            (SandstormCommandType::UpdateUser, result).write(writer).await?;
        }
        SandstormCommandType::DeleteUser => {
            let username = SmallReadString::read(reader).await?.0;
            let result = context.delete_user(username).await;
            (SandstormCommandType::DeleteUser, result).write(writer).await?;
        }
        SandstormCommandType::ListAuthMethods => {
            let auth_methods = context.get_auth_methods();
            (SandstormCommandType::ListAuthMethods, SmallWriteList(auth_methods.as_slice()))
                .write(writer)
                .await?;
        }
        SandstormCommandType::ToggleAuthMethod => {
            let auth_method = reader.read_u8().await?;
            let state = bool::read(reader).await?;
            let result = context.toggle_auth_method(auth_method, state).await;
            (SandstormCommandType::ToggleAuthMethod, result).write(writer).await?;
        }
        // SandstormCommandType::RequestCurrentMetrics => {}
        SandstormCommandType::GetBufferSize => {
            let buffer_size = context.get_buffer_size();
            (SandstormCommandType::GetBufferSize, buffer_size).write(writer).await?;
        }
        SandstormCommandType::SetBufferSize => {
            let buffer_size = reader.read_u32().await?;
            let result = context.set_buffer_size(buffer_size).await;
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
