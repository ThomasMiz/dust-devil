use std::{io, net::SocketAddr};

use dust_devil_core::{socks5::AuthMethod, u8_repr_enum::U8ReprEnum};
use tokio::io::{AsyncWrite, AsyncWriteExt};

#[repr(u8)]
pub enum SocksStatus {
    Success = 0,
    GeneralFailure = 1,
    ConnectionNotAllowed = 2,
    NetworkUnreachable = 3,
    HostUnreachable = 4,
    ConnectionRefused = 5,
    // TTLExpired = 6,
    CommandNotSupported = 7,
    AddressTypeNotSupported = 8,
}

pub async fn send_handshake_response<W>(writer: &mut W, method: Option<AuthMethod>) -> Result<(), io::Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    let buf = [0x05u8, method.map(|m| m.into_u8()).unwrap_or(0xFF)];
    writer.write_all(&buf).await
}

pub async fn send_request_response<W>(writer: &mut W, status: SocksStatus, socket_bound: Option<SocketAddr>) -> Result<(), std::io::Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    let mut buf = [0u8; 32];
    buf[0] = 5;
    buf[1] = status as u8;
    buf[2] = 0;

    let buf_len = match socket_bound {
        Some(SocketAddr::V4(ipv4)) => {
            buf[3] = 1;
            buf[4..8].copy_from_slice(&ipv4.ip().octets());

            let port = ipv4.port();
            buf[8] = (port >> 8) as u8;
            buf[9] = (port & 0x00FF) as u8;

            10
        }
        Some(SocketAddr::V6(ipv6)) => {
            buf[3] = 4;
            buf[4..20].copy_from_slice(&ipv6.ip().octets());

            let port = ipv6.port();
            buf[20] = (port >> 8) as u8;
            buf[21] = (port & 0x00FF) as u8;

            22
        }
        None => {
            buf[3] = 1;
            buf[4..10].fill(0);

            10
        }
    };

    writer.write_all(&buf[0..buf_len]).await
}
