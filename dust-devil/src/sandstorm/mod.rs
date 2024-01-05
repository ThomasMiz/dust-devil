use std::io;

use dust_devil_core::sandstorm::SandstormHandshakeStatus;
use tokio::{io::BufReader, net::TcpStream, select};
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

    Ok(())
}
