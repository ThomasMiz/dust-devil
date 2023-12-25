use std::{future::poll_fn, io, net::SocketAddr};

use tokio::net::{TcpListener, TcpStream};

pub async fn accept_from_any(listeners: &Vec<TcpListener>) -> Result<(TcpStream, SocketAddr), (&TcpListener, io::Error)> {
    poll_fn(|cx| {
        for l in listeners {
            let poll_status = l.poll_accept(cx);
            if let std::task::Poll::Ready(result) = poll_status {
                return std::task::Poll::Ready(match result {
                    Ok(result_ok) => Ok(result_ok),
                    Err(result_err) => Err((l, result_err)),
                });
            }
        }

        std::task::Poll::Pending
    })
    .await
}
