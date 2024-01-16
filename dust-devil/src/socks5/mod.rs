use std::{
    io::{self, ErrorKind},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};

use dust_devil_core::{
    socks5::{AuthMethod, SocksRequestAddress},
    u8_repr_enum::U8ReprEnum,
};
use tokio::{
    io::BufReader,
    net::{TcpSocket, TcpStream},
    select,
};
use tokio_util::sync::CancellationToken;

use crate::{
    context::ClientContext,
    log_socks_connect_to_destination_failed, log_socks_connected_to_destination, log_socks_connection_attempt,
    log_socks_connection_attempt_bind_failed, log_socks_connection_attempt_connect_failed, log_socks_dns_lookup, log_socks_finished,
    log_socks_selected_auth, log_socks_unsupported_atyp, log_socks_unsupported_command, log_socks_unsupported_version,
    socks5::{
        parsers::{parse_handshake, parse_request},
        responses::{send_handshake_response, send_request_response},
    },
};

mod auth;
mod copy;
mod parsers;
mod responses;

use auth::*;
use parsers::*;
use responses::*;

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
        result = handle_socks5_inner(stream, &mut context) => log_socks_finished!(context, result),
        _ = cancel_token.cancelled() => {}
    }
}

async fn handle_socks5_inner(mut stream: TcpStream, context: &mut ClientContext) -> Result<(), io::Error> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::with_capacity(context.buffer_size(), reader);
    let maybe_auth_method = match parse_handshake(&mut reader).await {
        Ok(handshake) => select_auth_method(context, &handshake.methods),
        Err(ParseHandshakeError::IO(error)) => return Err(error),
        Err(ParseHandshakeError::InvalidVersion(ver)) => {
            log_socks_unsupported_version!(context, ver);
            send_handshake_response(&mut writer, None).await?;
            return Ok(());
        }
    };

    log_socks_selected_auth!(context, maybe_auth_method);
    send_handshake_response(&mut writer, maybe_auth_method).await?;

    let auth_status = match maybe_auth_method {
        Some(AuthMethod::NoAuth) => true,
        Some(AuthMethod::UsernameAndPassword) => handle_userpass_auth(&mut reader, &mut writer, context).await?,
        None => false,
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
                log_socks_dns_lookup!(context, domainname.clone());
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
                ParseRequestError::InvalidVersion(ver) => log_socks_unsupported_version!(context, ver),
                ParseRequestError::CommandNotSupported(cmd) => log_socks_unsupported_command!(context, cmd),
                ParseRequestError::InvalidATYP(atyp) => log_socks_unsupported_atyp!(context, atyp),
            }

            send_request_response(&mut writer, error.into(), None).await?;
            return Ok(());
        }
    };

    let mut destination_stream = match connect_socket(request_addresses, context).await {
        Ok(stream) => stream,
        Err(status) => {
            log_socks_connect_to_destination_failed!(context);
            send_request_response(&mut writer, status, None).await?;
            return Ok(());
        }
    };

    send_request_response(&mut writer, SocksStatus::Success, destination_stream.local_addr().ok()).await?;

    let (dst_reader, mut dst_writer) = destination_stream.split();
    let mut dst_reader = BufReader::with_capacity(context.buffer_size(), dst_reader);

    copy::copy_bidirectional(&mut reader, &mut writer, &mut dst_reader, &mut dst_writer, context).await
}

fn select_auth_method(state: &ClientContext, methods: &[u8]) -> Option<AuthMethod> {
    if state.is_noauth_enabled() && methods.contains(&AuthMethod::NoAuth.into_u8()) {
        Some(AuthMethod::NoAuth)
    } else if state.is_userpass_enabled() && methods.contains(&AuthMethod::UsernameAndPassword.into_u8()) {
        Some(AuthMethod::UsernameAndPassword)
    } else {
        None
    }
}

async fn connect_socket(request_addresses: Vec<SocketAddr>, context: &ClientContext) -> Result<TcpStream, SocksStatus> {
    let mut last_error = None;

    for address in request_addresses {
        log_socks_connection_attempt!(context, address);

        let destination_socket = match address {
            SocketAddr::V4(_) => TcpSocket::new_v4(),
            SocketAddr::V6(_) => TcpSocket::new_v6(),
        };

        let destination_socket = match destination_socket {
            Ok(dst_socket) => dst_socket,
            Err(error) => {
                log_socks_connection_attempt_bind_failed!(context, error);
                continue;
            }
        };

        let destination_stream = match destination_socket.connect(address).await {
            Ok(dst_stream) => dst_stream,
            Err(error) => {
                last_error = Some(SocksStatus::from(&error));
                log_socks_connection_attempt_connect_failed!(context, error);
                continue;
            }
        };

        log_socks_connected_to_destination!(context, address);
        return Ok(destination_stream);
    }

    Err(last_error.unwrap_or(SocksStatus::HostUnreachable))
}
