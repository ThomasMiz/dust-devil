use std::io;

use dust_devil_core::{
    sandstorm::{ParseHandshakeError, SandstormHandshake, SandstormHandshakeStatus},
    serialize::ByteWrite,
};
use tokio::{
    io::{AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
    select,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;

use crate::{
    context::SandstormContext,
    log_sandstorm_authenticated_as, log_sandstorm_finished, log_sandstorm_unsupported_version,
    sandstorm::{request_handler::handle_requests, response_handler::handle_responses},
};

mod error_handling;
mod messaging;
mod request_handler;
mod response_handler;

const SANDSTORM_READ_BUFFER_SIZE: usize = 1024;
const SANDSTORM_WRITE_BUFFER_SIZE: usize = 1024;
const RESPONSE_NOTIFICATION_CHANNEL_SIZE: usize = 16;

pub async fn handle_sandstorm(stream: TcpStream, mut context: SandstormContext, cancel_token: CancellationToken) {
    select! {
        result = handle_sandstorm_inner(stream, &mut context) => log_sandstorm_finished!(context, result),
        _ = cancel_token.cancelled() => {}
    }
}

async fn handle_sandstorm_inner(mut stream: TcpStream, context: &mut SandstormContext) -> Result<(), io::Error> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::with_capacity(SANDSTORM_READ_BUFFER_SIZE, reader);

    let handshake = match SandstormHandshake::read_with_version_check(&mut reader).await {
        Ok(handshake) => handshake,
        Err(ParseHandshakeError::IO(error)) => return Err(error),
        Err(ParseHandshakeError::InvalidVersion(ver)) => {
            log_sandstorm_unsupported_version!(context, ver);
            SandstormHandshakeStatus::UnsupportedVersion.write(&mut writer).await?;
            let _ = writer.shutdown().await;
            return Ok(());
        }
    };

    let success = context.try_login(&handshake.username, &handshake.password);
    log_sandstorm_authenticated_as!(context, handshake.username, success == Some(true));

    let handshake_response = match success {
        Some(true) => SandstormHandshakeStatus::Ok,
        Some(false) => SandstormHandshakeStatus::PermissionDenied,
        None => SandstormHandshakeStatus::InvalidUsernameOrPassword,
    };
    handshake_response.write(&mut writer).await?;

    let mut writer = BufWriter::with_capacity(SANDSTORM_WRITE_BUFFER_SIZE, writer);
    let (response_tx, response_rx) = mpsc::channel(RESPONSE_NOTIFICATION_CHANNEL_SIZE);

    select! {
        biased;
        request_result = handle_requests(&mut reader, context, response_tx) => request_result,
        request_response = handle_responses(&mut writer, context, response_rx) => request_response,
    }
}
