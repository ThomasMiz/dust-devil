use std::{
    io::{self, ErrorKind},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
    sync::Arc,
};

use tokio::{
    io::BufReader,
    net::{TcpSocket, TcpStream},
};

use crate::{socks5::{
    parsers::{parse_handshake, parse_request, SocksRequestAddress},
    responses::{send_handshake_response, send_request_response},
}, server::ServerState};

mod auth;
mod chunk_reader;
mod copy;
mod parsers;
mod responses;

use auth::*;
use parsers::*;
use responses::*;

impl From<io::Error> for responses::SocksStatus {
    fn from(value: io::Error) -> Self {
        match value.kind() {
            ErrorKind::ConnectionAborted | ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset => SocksStatus::ConnectionRefused,
            ErrorKind::NotConnected => SocksStatus::NetworkUnreachable,
            ErrorKind::PermissionDenied => SocksStatus::ConnectionNotAllowed,
            ErrorKind::TimedOut => SocksStatus::HostUnreachable,
            ErrorKind::AddrNotAvailable | ErrorKind::Unsupported => SocksStatus::AddressTypeNotSupported,
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

pub async fn handle_socks5(stream: TcpStream, client_id: u64, state: Arc<ServerState>) {
    match handle_socks5_inner(stream, client_id, state).await {
        Ok(()) => println!("Client {client_id} connection closed"),
        Err(error) => println!("Client {client_id} closed with IO error: {error}"),
    }
}

async fn handle_socks5_inner(mut stream: TcpStream, client_id: u64, state: Arc<ServerState>) -> Result<(), io::Error> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    println!("Client {client_id} doing handshake...");
    let auth_method = match parse_handshake(&mut reader).await {
        Ok(handshake) => select_auth_method(&state, &handshake.methods),
        Err(SocksError::IO(error)) => return Err(error),
        Err(SocksError::InvalidVersion(ver)) => {
            println!("Client {client_id} requested unsupported socks version: {ver}");
            send_handshake_response(&mut writer, AuthMethod::NoAcceptableMethod).await?;
            return Ok(());
        }
        Err(error) => {
            println!("Client {client_id} error during handshake: {error:?}");
            return Ok(());
        }
    };

    if auth_method == AuthMethod::NoAcceptableMethod {
        println!("Client {client_id} no acceptable authentication method found");
    } else {
        println!("Client {client_id} will use auth method: {auth_method:?}");
    }
    send_handshake_response(&mut writer, auth_method).await?;

    let auth_status = match auth_method {
        AuthMethod::NoAuth => true,
        AuthMethod::UsernameAndPassword => handle_userpass_auth(&mut reader, &mut writer, &state, client_id).await?,
        _ => false,
    };

    if !auth_status {
        return Ok(());
    }

    println!("Client {client_id} doing request...");
    let request_addresses = match parse_request(&mut reader).await {
        Ok(request) => match request.destination {
            SocksRequestAddress::IPv4(ipv4) => {
                vec![SocketAddr::V4(SocketAddrV4::new(ipv4, request.port))]
            }
            SocksRequestAddress::IPv6(ipv6) => {
                vec![SocketAddr::V6(SocketAddrV6::new(ipv6, request.port, 0, 0))]
            }
            SocksRequestAddress::Domainname(mut domain) => {
                println!("Client {client_id} looking up DNS resolution for {domain}");
                domain.push_str(":0");

                tokio::net::lookup_host(domain)
                    .await?
                    .map(|mut x| {
                        x.set_port(request.port);
                        x
                    })
                    .collect()
            }
        },
        Err(SocksError::IO(error)) => return Err(error),
        Err(error) => {
            println!("Client {client_id} error during request: {error:?}");
            send_request_response(&mut writer, error.into(), None).await?;
            return Ok(());
        }
    };

    let mut destination_stream = match connect_socket(request_addresses, client_id).await {
        Ok(stream) => stream,
        Err(status) => {
            println!("Client {client_id} failed to connect to remote");
            send_request_response(&mut writer, status, None).await?;
            return Ok(());
        }
    };

    send_request_response(&mut writer, SocksStatus::Success, destination_stream.local_addr().ok()).await?;

    println!("Client {client_id} doing the copy thingy...");
    let (dst_reader, mut dst_writer) = destination_stream.split();
    let mut dst_reader = BufReader::new(dst_reader);

    let result = copy::copy_bidirectional(&mut reader, &mut writer, &mut dst_reader, &mut dst_writer, client_id).await;
    match result {
        Ok((client_to_remote, remote_to_client)) => {
            println!("Client {client_id} finished after {client_to_remote} bytes sent and {remote_to_client} bytes received");
        }
        Err(error) => {
            println!("Client {client_id} post-socks error: {error:?}");
        }
    }

    Ok(())
}

fn select_auth_method(state: &Arc<ServerState>, methods: &[u8]) -> AuthMethod {
    if state.no_auth_enabled && methods.contains(&(AuthMethod::NoAuth as u8)) {
        AuthMethod::NoAuth
    } else if state.userpass_auth_enabled && methods.contains(&(AuthMethod::UsernameAndPassword as u8)) {
        AuthMethod::UsernameAndPassword
    } else {
        AuthMethod::NoAcceptableMethod
    }
}

async fn connect_socket(request_addresses: Vec<SocketAddr>, client_id: u64) -> Result<TcpStream, SocksStatus> {
    let mut last_error = None;

    for address in request_addresses {
        println!("Client {client_id} connecting to destination {address}");
        let destination_socket = if address.is_ipv4() {
            TcpSocket::new_v4()
        } else if address.is_ipv6() {
            TcpSocket::new_v6()
        } else {
            continue;
        };

        let destination_socket = match destination_socket {
            Ok(dst_socket) => dst_socket,
            Err(error) => {
                println!("Client {client_id} failed to bind local socket: {error:?}");
                continue;
            }
        };

        let destination_stream = match destination_socket.connect(address).await {
            Ok(dst_stream) => dst_stream,
            Err(error) => {
                println!("Client {client_id} failed to connect to remote: {error:?}");
                last_error = Some(error);
                continue;
            }
        };

        println!("Client {client_id} connection established to {}", address);
        return Ok(destination_stream);
    }

    match last_error {
        Some(error) => Err(SocksStatus::from(error)),
        None => Err(SocksStatus::HostUnreachable),
    }
}
