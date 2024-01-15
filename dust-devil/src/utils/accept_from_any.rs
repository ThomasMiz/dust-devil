use std::{future::poll_fn, io, net::SocketAddr, task::Poll};

use tokio::net::{TcpListener, TcpStream};

pub async fn accept_from_any(listeners: &Vec<TcpListener>) -> Result<(TcpStream, SocketAddr), (&TcpListener, io::Error)> {
    poll_fn(|cx| {
        for l in listeners {
            let poll_status = l.poll_accept(cx);
            if let Poll::Ready(result) = poll_status {
                return Poll::Ready(match result {
                    Ok(result_ok) => Ok(result_ok),
                    Err(result_err) => Err((l, result_err)),
                });
            }
        }

        Poll::Pending
    })
    .await
}
