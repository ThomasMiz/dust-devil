use std::io;

use tokio::{net::TcpStream, select};
use tokio_util::sync::CancellationToken;

use crate::context::SandstormContext;

pub async fn handle_sandstorm(stream: TcpStream, mut context: SandstormContext, cancel_token: CancellationToken) {
    select! {
        result = handle_sandstorm_inner(stream, &mut context) => context.log_finished(result).await,
        _ = cancel_token.cancelled() => {}
    }
}

async fn handle_sandstorm_inner(mut stream: TcpStream, _context: &mut SandstormContext) -> Result<(), io::Error> {
    let (mut reader, mut writer) = stream.split();
    let result = tokio::io::copy(&mut reader, &mut writer).await;
    result.map(|x| ())
}
