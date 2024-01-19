use std::io::{Error, ErrorKind};

use dust_devil_core::sandstorm::{EventStreamConfigResponse, EventStreamResponse};
use time::{OffsetDateTime, UtcOffset};
use tokio::{io::AsyncWrite, sync::oneshot};

use crate::{printlnif, sandstorm::SandstormRequestManager};

fn get_event_handler_fn() -> impl FnMut(EventStreamResponse) {
    let utc_offset = match UtcOffset::current_local_offset() {
        Ok(offset) => offset,
        Err(_) => {
            eprintln!("Could not determine system's UTC offset, defaulting to 00:00:00");
            UtcOffset::UTC
        }
    };

    move |event| {
        let event = event.0;
        let t = OffsetDateTime::from_unix_timestamp(event.timestamp)
            .map(|t| t.to_offset(utc_offset))
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);

        println!(
            "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}] {}",
            t.year(),
            t.month() as u8,
            t.day(),
            t.hour(),
            t.minute(),
            t.second(),
            event.data
        );
    }
}

pub async fn handle_output<W>(verbose: bool, manager: &mut SandstormRequestManager<W>) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    printlnif!(verbose, "Enabling event stream");
    let (tx, rx) = oneshot::channel();
    manager
        .event_stream_config_fn(true, |result| match result {
            EventStreamConfigResponse::Disabled => {
                eprintln!("Server refused to enable event streaming");
                let _ = tx.send(false);
                None
            }
            EventStreamConfigResponse::Enabled(metrics) => {
                println!("Event streaming enabled! Current metrics are:");
                println!("current client connections: {}", metrics.current_client_connections);
                println!("historic client connections: {}", metrics.historic_client_connections);
                println!("client bytes sent: {}", metrics.client_bytes_sent);
                println!("client bytes received: {}", metrics.client_bytes_received);
                println!("current sandstorm connections: {}", metrics.current_sandstorm_connections);
                println!("historic sandstorm connections: {}", metrics.historic_sandstorm_connections);
                let _ = tx.send(true);
                Some(Box::new(get_event_handler_fn()))
            }
            EventStreamConfigResponse::WasAlreadyEnabled => {
                eprintln!("Couldn't enable event streaming: Server responded with WasAlreadyEnabled.");
                let _ = tx.send(false);
                None
            }
        })
        .await?;

    manager.flush_writer().await?;

    let status = rx
        .await
        .map_err(|_| Error::new(ErrorKind::Other, "Could not receive response from manager. Is manager closing?"))?;

    if status {
        tokio::signal::ctrl_c().await?;
        println!("Received break signal, shutting down gracefully");
    }

    Ok(())
}
