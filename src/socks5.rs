use std::{
    io::{self, ErrorKind},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpSocket, TcpStream,
    },
};

#[repr(u8)]
enum SocksStatus {
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

#[derive(Debug)]
enum SocksError {
    InvalidVersion,
    NoAcceptableAuthMethod,
    CommandNotSupported,
    InvalidATYP,
    IO(io::Error),
}

#[repr(u8)]
enum AuthMethod {
    NoAuth = 0,
}

impl From<io::Error> for SocksStatus {
    fn from(value: io::Error) -> Self {
        match value.kind() {
            ErrorKind::ConnectionAborted | ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset => SocksStatus::ConnectionRefused,
            ErrorKind::NotConnected => SocksStatus::NetworkUnreachable,
            ErrorKind::PermissionDenied => SocksStatus::ConnectionNotAllowed,
            ErrorKind::TimedOut => SocksStatus::HostUnreachable,
            ErrorKind::AddrNotAvailable | ErrorKind::Unsupported=> SocksStatus::AddressTypeNotSupported,
            _ => SocksStatus::GeneralFailure,
        }
    }
}

impl From<SocksError> for SocksStatus {
    fn from(value: SocksError) -> Self {
        match value {
            SocksError::CommandNotSupported => SocksStatus::CommandNotSupported,
            SocksError::InvalidATYP => SocksStatus::AddressTypeNotSupported,
            SocksError::IO(error) => error.into(),
            _ => SocksStatus::GeneralFailure,
        }
    }
}

impl From<io::Error> for SocksError {
    fn from(value: io::Error) -> Self {
        SocksError::IO(value)
    }
}

pub async fn handle_socks5(mut stream: TcpStream, client_id: usize) {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    println!("Client {client_id} doing handshake...");
    let _auth_method = match read_handshake(&mut reader).await {
        Ok(auth_method) => {
            if let Err(error) = writer.write_all(b"\x05\x00").await {
                println!("Client {client_id} failed to send back successful handshake response: {error:?}");
                return;
            }

            auth_method
        },
        Err(SocksError::IO(error)) => {
            println!("Client {client_id} IO error during handshake: {error:?}");
            return;
        },
        Err(error) => {
            println!("Client {client_id} error during handshake: {error:?}");
            if let Err(error) = writer.write_all(b"\x05\xFF").await {
                println!("Client {client_id} error during handshake: Couldn't send response with no auth method: {error:?}");
            }
            return;
        },
    };

    println!("Client {client_id} doing request...");
    let request_address = match read_request(&mut reader).await {
        Ok(request) => request,
        Err(SocksError::IO(error)) => {
            println!("Client {client_id} IO error during request: {error:?}");
            return;
        },
        Err(error) => {
            println!("Client {client_id} error during request: {error:?}");
            if let Err(error2) = send_response(&mut writer, error.into(), None).await {
                println!("Client {client_id} failed to send back failure response: {error2:?}");
            }
            return;
        },
    };

    println!("Client {client_id} connecting to destination...");
    let destination_socket = match TcpSocket::new_v4() {
        Ok(dst_socket) => dst_socket,
        Err(error) => {
            println!("Client {client_id} failed to bind local socket: {error:?}");
            if let Err(error2) = send_response(&mut writer, error.into(), None).await {
                println!("Client {client_id} failed to send back failure response: {error2:?}");
            }
            return;
        },
    };

    let mut destination_stream = match destination_socket.connect(request_address).await {
        Ok(dst_stream) => dst_stream,
        Err(error) => {
            println!("Client {client_id} failed to connect to remote: {error:?}");
            if let Err(error2) = send_response(&mut writer, error.into(), None).await {
                println!("Client {client_id} failed to send back failure response: {error2:?}");
            }
            return;
        },
    };

    println!("Client {client_id} sending back successful response...");
    if let Err(error) = send_response(&mut writer, SocksStatus::Success, destination_stream.local_addr().ok()).await {
        println!("Client {client_id} failed to send back response after successful connection: {error:?}");
        return;
    }

    println!("Client {client_id} doing the copy thingy...");
    let result = tokio::io::copy_bidirectional(&mut stream, &mut destination_stream).await;
    match result {
        Ok((client_to_remote, remote_to_client)) => {
            println!("Client {client_id} finished after {client_to_remote} bytes sent and {remote_to_client} bytes received");
        },
        Err(error) => {
            println!("Client {client_id} post-socks error: {error:?}");
        },
    }
}

async fn read_handshake(reader: &mut BufReader<ReadHalf<'_>>) -> Result<AuthMethod, SocksError> {
    let ver = reader.read_u8().await?;
    if ver != 5 {
        return Err(SocksError::InvalidVersion);
    }

    let nmethods = reader.read_u8().await?;
    let mut methods = [0u8; 255];

    reader.read_exact(&mut methods[0..(nmethods as usize)]).await?;

    if methods.contains(&0) {
        Ok(AuthMethod::NoAuth)
    } else {
        Err(SocksError::NoAcceptableAuthMethod)
    }
}

async fn read_request(reader: &mut BufReader<ReadHalf<'_>>) -> Result<SocketAddr, SocksError> {
    let ver = reader.read_u8().await?;
    if ver != 5 {
        return Err(SocksError::InvalidVersion);
    }

    let cmd = reader.read_u8().await?;
    if cmd != 1 {
        return Err(SocksError::CommandNotSupported)
    }

    let _rsv = reader.read_u8().await?;
    let atyp = reader.read_u8().await?;

    let address = match atyp {
        1 => {
            let mut octets = [0u8; 4];
            reader.read_exact(&mut octets).await?;
            let port = reader.read_u16().await?;

            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(octets), port))
        },
        /*3 => {
            let length = reader.read_u8().await?();
            // TODO: Implement domain ATYP
        }*/
        4 => {
            let mut octets = [0u8; 16];
            reader.read_exact(&mut octets).await?;
            let port = reader.read_u16().await?;

            SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(octets), port, 0, 0))
        }
        _ => {
            return Err(SocksError::InvalidATYP);
        }
    };

    Ok(address)
}

async fn send_response(writer: &mut WriteHalf<'_>, status: SocksStatus, socket_bound: Option<SocketAddr>) -> Result<(), std::io::Error> {
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
