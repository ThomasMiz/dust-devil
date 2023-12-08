use std::{net::{SocketAddr, Ipv6Addr, SocketAddrV4, Ipv4Addr, SocketAddrV6}, io::ErrorKind};

use tokio::{net::{TcpListener, TcpStream, tcp::WriteHalf, TcpSocket}, io::{AsyncReadExt, AsyncWriteExt, BufReader}, select};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let bind_address = "localhost:1080";

    let listener = match TcpListener::bind(bind_address).await {
        Ok(result) => result,
        Err(_) => {
            println!("Failed to set up socket at {bind_address}");
            return;
        },
    };

    loop {
        select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((socket, address)) => {
                        println!("Accepted new connection from {}", address);
                        tokio::spawn(async move {
                            handle_socks5(socket).await;
                        });
                    },
                    Err(err) => {
                        println!("Error while accepting new connection: {}", err);
                    },
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("Goodbye");
                break;
            },
        }
    }
}

#[repr(u8)]
enum SocksStatus {
    Success = 0,
    GeneralFailure = 1,
    ConnectionNotAllowed = 2,
    NetworkUnreachable = 3,
    HostUnreachable = 4,
    ConnectionRefused = 5,
    TTLExpired = 6,
    CommandNotSupported = 7,
    AddressTypeNotSupported = 8,
}

async fn handle_socks5(mut socket: TcpStream) {
    let (reader, mut writer) = socket.split();

    let mut reader = BufReader::new(reader);

    let ver = reader.read_u8().await.unwrap();
    let nmethods = reader.read_u8().await.unwrap();
    let mut methods = [0u8; 255];

    reader.read_exact(&mut methods[0..(nmethods as usize)]).await.unwrap();

    if ver != 5 || !methods.contains(&0) {
        writer.write_all(b"\x05\xFF").await.unwrap();
        writer.shutdown().await.unwrap();
        return;
    }

    writer.write_all(b"\x05\x00").await.unwrap();

    let ver = reader.read_u8().await.unwrap();
    if ver != 5 {
        send_response(&mut writer, SocksStatus::GeneralFailure, None).await.unwrap();
        return;
    }

    let cmd = reader.read_u8().await.unwrap();
    if cmd != 1 {
        send_response(&mut writer, SocksStatus::CommandNotSupported, None).await.unwrap();
        return;
    }

    let _rsv = reader.read_u8().await.unwrap();

    let atyp = reader.read_u8().await.unwrap();

    let address = match atyp {
        1 => {
            let mut octets = [0u8; 4];
            reader.read_exact(&mut octets).await.unwrap();
            let port = reader.read_u16().await.unwrap();

            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(octets), port))
        },
        /*3 => {
            let length = reader.read_u8().await.unwrap();
            // TODO: Implement domain ATYP
        }*/
        4 => {
            let mut octets = [0u8; 16];
            reader.read_exact(&mut octets).await.unwrap();
            let port = reader.read_u16().await.unwrap();

            SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(octets), port, 0, 0))
        }
        _ => {
            send_response(&mut writer, SocksStatus::AddressTypeNotSupported, None).await.unwrap();
            return;
        }
    };

    let destination_socket = TcpSocket::new_v4().unwrap();
    let connect_result: Result<TcpStream, std::io::Error> = destination_socket.connect(address).await;
    match connect_result {
        Err(err) => {
            println!("{err}");
            let status = match err.kind() {
                ErrorKind::ConnectionAborted | ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset => SocksStatus::ConnectionRefused,
                ErrorKind::NotConnected => SocksStatus::NetworkUnreachable,
                ErrorKind::PermissionDenied => SocksStatus::ConnectionNotAllowed,
                ErrorKind::TimedOut => SocksStatus::HostUnreachable,
                _ => SocksStatus::GeneralFailure,
            };

            send_response(&mut writer, status, None).await.unwrap();
        },
        Ok(mut destination_stream) => {
            send_response(&mut writer, SocksStatus::Success, destination_stream.local_addr().ok()).await.unwrap();
            tokio::io::copy_bidirectional(&mut socket, &mut destination_stream).await.unwrap();
        },
    }
}

async fn send_response<'a>(writer: &'a mut WriteHalf<'a>, status: SocksStatus, socket_bound: Option<SocketAddr>) -> Result<(), std::io::Error> {
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
        },
        Some(SocketAddr::V6(ipv6)) => {
            buf[3] = 4;
            buf[4..20].copy_from_slice(&ipv6.ip().octets());

            let port = ipv6.port();
            buf[20] = (port >> 8) as u8;
            buf[21] = (port & 0x00FF) as u8;

            22
        },
        None => {
            buf[3] = 1;
            buf[4..10].fill(0);

            10
        },
    };

    writer.write_all(&buf[0..buf_len]).await
}
