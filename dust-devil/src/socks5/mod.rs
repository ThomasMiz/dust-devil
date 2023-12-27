use std::{
    io::{self, ErrorKind},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};

use dust_devil_core::socks5::{AuthMethod, SocksRequestAddress};
use tokio::{
    io::BufReader,
    net::{TcpSocket, TcpStream},
    select,
};
use tokio_util::sync::CancellationToken;

use crate::{
    context::ClientContext,
    socks5::{
        parsers::{parse_handshake, parse_request},
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

const SOCKS_BUFFER_SIZE: usize = 0x2000;

impl From<&io::Error> for responses::SocksStatus {
    fn from(value: &io::Error) -> Self {
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

impl From<ParseRequestError> for SocksStatus {
    fn from(value: ParseRequestError) -> Self {
        match value {
            ParseRequestError::CommandNotSupported(_) => SocksStatus::CommandNotSupported,
            ParseRequestError::InvalidATYP(_) => SocksStatus::AddressTypeNotSupported,
            ParseRequestError::IO(error) => (&error).into(),
            _ => SocksStatus::GeneralFailure,
        }
    }
}

pub async fn handle_socks5(stream: TcpStream, mut context: ClientContext, cancel_token: CancellationToken) {
    select! {
        result = handle_socks5_inner(stream, &mut context) => context.log_finished(result).await,
        _ = cancel_token.cancelled() => {}
    }
}

async fn handle_socks5_inner(mut stream: TcpStream, context: &mut ClientContext) -> Result<(), io::Error> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::with_capacity(SOCKS_BUFFER_SIZE, reader);
    let auth_method = match parse_handshake(&mut reader).await {
        Ok(handshake) => select_auth_method(context, &handshake.methods),
        Err(ParseHandshakeError::IO(error)) => return Err(error),
        Err(ParseHandshakeError::InvalidVersion(ver)) => {
            context.log_unsupported_socks_version(ver).await;
            send_handshake_response(&mut writer, AuthMethod::NoAcceptableMethod).await?;
            return Ok(());
        }
    };

    context.log_selected_auth(auth_method).await;
    send_handshake_response(&mut writer, auth_method).await?;

    let auth_status = match auth_method {
        AuthMethod::NoAuth => true,
        AuthMethod::UsernameAndPassword => handle_userpass_auth(&mut reader, &mut writer, context).await?,
        _ => false,
    };

    if !auth_status {
        return Ok(());
    }

    let request_addresses = match parse_request(&mut reader).await {
        Ok(request) => match request.destination {
            SocksRequestAddress::IPv4(ipv4) => {
                vec![SocketAddr::V4(SocketAddrV4::new(ipv4, request.port))]
            }
            SocksRequestAddress::IPv6(ipv6) => {
                vec![SocketAddr::V6(SocketAddrV6::new(ipv6, request.port, 0, 0))]
            }
            SocksRequestAddress::Domainname(mut domainname) => {
                context.log_dns_lookup(domainname.clone()).await;
                domainname.push_str(":0");

                tokio::net::lookup_host(domainname)
                    .await?
                    .map(|mut x| {
                        x.set_port(request.port);
                        x
                    })
                    .collect()
            }
        },
        Err(ParseRequestError::IO(error)) => return Err(error),
        Err(error) => {
            match error {
                ParseRequestError::IO(error) => return Err(error),
                ParseRequestError::InvalidVersion(ver) => context.log_unsupported_socks_version(ver).await,
                ParseRequestError::CommandNotSupported(cmd) => context.log_unsupported_socks_command(cmd).await,
                ParseRequestError::InvalidATYP(atyp) => context.log_unsupported_atyp(atyp).await,
            }

            send_request_response(&mut writer, error.into(), None).await?;
            return Ok(());
        }
    };

    let mut destination_stream = match connect_socket(request_addresses, context).await {
        Ok(stream) => stream,
        Err(status) => {
            context.log_connect_to_destination_failed().await;
            send_request_response(&mut writer, status, None).await?;
            return Ok(());
        }
    };

    send_request_response(&mut writer, SocksStatus::Success, destination_stream.local_addr().ok()).await?;

    let (dst_reader, mut dst_writer) = destination_stream.split();
    let mut dst_reader = BufReader::with_capacity(SOCKS_BUFFER_SIZE, dst_reader);

    copy::copy_bidirectional(&mut reader, &mut writer, &mut dst_reader, &mut dst_writer, context).await
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
        context.log_connection_attempt(address).await;

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
                context.log_connection_attempt_bind_failed(error).await;
                continue;
            }
        };

        let destination_stream = match destination_socket.connect(address).await {
            Ok(dst_stream) => dst_stream,
            Err(error) => {
                last_error = Some(SocksStatus::from(&error));
                context.log_connection_attempt_connect_failed(error).await;
                continue;
            }
        };

        context.log_connected_to_destination(address).await;
        return Ok(destination_stream);
    }

    Err(last_error.unwrap_or(SocksStatus::HostUnreachable))
}
