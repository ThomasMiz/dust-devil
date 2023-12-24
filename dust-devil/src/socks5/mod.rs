use std::{
    io::{self, ErrorKind},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};

use tokio::{
    io::BufReader,
    net::{TcpSocket, TcpStream},
};

use crate::{
    context::ClientContext,
    socks5::{
        parsers::{parse_handshake, parse_request, SocksRequestAddress},
        responses::{send_handshake_response, send_request_response},
    },
};

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

pub async fn handle_socks5(stream: TcpStream, context: ClientContext) {
    match handle_socks5_inner(stream, &context).await {
        Ok(()) => println!("Client {} connection closed", context.client_id()),
        Err(error) => println!("Client {} closed with IO error: {error}", context.client_id()),
    }
}

async fn handle_socks5_inner(mut stream: TcpStream, context: &ClientContext) -> Result<(), io::Error> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    println!("Client {} doing handshake...", context.client_id());
    let auth_method = match parse_handshake(&mut reader).await {
        Ok(handshake) => select_auth_method(context, &handshake.methods),
        Err(SocksError::IO(error)) => return Err(error),
        Err(SocksError::InvalidVersion(ver)) => {
            println!("Client {} requested unsupported socks version: {ver}", context.client_id());
            send_handshake_response(&mut writer, AuthMethod::NoAcceptableMethod).await?;
            return Ok(());
        }
        Err(error) => {
            println!("Client {} error during handshake: {error:?}", context.client_id());
            return Ok(());
        }
    };

    if auth_method == AuthMethod::NoAcceptableMethod {
        println!("Client {} no acceptable authentication method found", context.client_id());
    } else {
        println!("Client {} will use auth method: {auth_method:?}", context.client_id());
    }
    send_handshake_response(&mut writer, auth_method).await?;

    let auth_status = match auth_method {
        AuthMethod::NoAuth => true,
        AuthMethod::UsernameAndPassword => handle_userpass_auth(&mut reader, &mut writer, context).await?,
        _ => false,
    };

    if !auth_status {
        return Ok(());
    }

    println!("Client {} doing request...", context.client_id());
    let request_addresses = match parse_request(&mut reader).await {
        Ok(request) => match request.destination {
            SocksRequestAddress::IPv4(ipv4) => {
                vec![SocketAddr::V4(SocketAddrV4::new(ipv4, request.port))]
            }
            SocksRequestAddress::IPv6(ipv6) => {
                vec![SocketAddr::V6(SocketAddrV6::new(ipv6, request.port, 0, 0))]
            }
            SocksRequestAddress::Domainname(mut domain) => {
                println!("Client {} looking up DNS resolution for {domain}", context.client_id());
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
            println!("Client {} error during request: {error:?}", context.client_id());
            send_request_response(&mut writer, error.into(), None).await?;
            return Ok(());
        }
    };

    let mut destination_stream = match connect_socket(request_addresses, context).await {
        Ok(stream) => stream,
        Err(status) => {
            println!("Client {} failed to connect to remote", context.client_id());
            send_request_response(&mut writer, status, None).await?;
            return Ok(());
        }
    };

    send_request_response(&mut writer, SocksStatus::Success, destination_stream.local_addr().ok()).await?;

    println!("Client {} doing the copy thingy...", context.client_id());
    let (dst_reader, mut dst_writer) = destination_stream.split();
    let mut dst_reader = BufReader::new(dst_reader);

    let result = copy::copy_bidirectional(&mut reader, &mut writer, &mut dst_reader, &mut dst_writer, context).await;
    match result {
        Ok((client_to_remote, remote_to_client)) => {
            println!("Client {} finished after {client_to_remote} bytes sent and {remote_to_client} bytes received", context.client_id());
        }
        Err(error) => {
            println!("Client {} post-socks error: {error:?}", context.client_id());
        }
    }

    Ok(())
}

fn select_auth_method(state: &ClientContext, methods: &[u8]) -> AuthMethod {
    if state.is_noauth_enabled() && methods.contains(&(AuthMethod::NoAuth as u8)) {
        AuthMethod::NoAuth
    } else if state.is_userpass_enabled() && methods.contains(&(AuthMethod::UsernameAndPassword as u8)) {
        AuthMethod::UsernameAndPassword
    } else {
        AuthMethod::NoAcceptableMethod
    }
}

async fn connect_socket(request_addresses: Vec<SocketAddr>, context: &ClientContext) -> Result<TcpStream, SocksStatus> {
    let mut last_error = None;

    for address in request_addresses {
        println!("Client {} connecting to destination {address}", context.client_id());
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
                println!("Client {} failed to bind local socket: {error:?}", context.client_id());
                continue;
            }
        };

        let destination_stream = match destination_socket.connect(address).await {
            Ok(dst_stream) => dst_stream,
            Err(error) => {
                println!("Client {} failed to connect to remote: {error:?}", context.client_id());
                last_error = Some(error);
                continue;
            }
        };

        println!("Client {} connection established to {}", context.client_id(), address);
        return Ok(destination_stream);
    }

    match last_error {
        Some(error) => Err(SocksStatus::from(error)),
        None => Err(SocksStatus::HostUnreachable),
    }
}
