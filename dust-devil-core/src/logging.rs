use core::fmt;
use std::{io, net::SocketAddr};

use crate::{
    socks5::{AuthMethod, SocksRequest, SocksRequestAddress},
    users::{UserRole, UsersLoadingError, DEFAULT_USER_PASSWORD, DEFAULT_USER_USERNAME},
};

pub enum LogEvent {
    NewListeningSocket(SocketAddr),
    FailedBindListeningSocket(SocketAddr, io::Error),
    FailedBindAnySocketAborting,
    RemovedListeningSocket(SocketAddr),
    LoadingUsersFromFile(String),
    UsersLoadedFromFile(String, Result<u64, UsersLoadingError>),
    StartingUpWithSingleDefaultUser,
    SavingUsersToFile(String),
    UsersSavedToFile(String, Result<u64, io::Error>),
    UserRegistered(String, UserRole),
    UserReplacedByArgs(String, UserRole),
    UserUpdated(String, UserRole, bool),
    UserDeleted(String, UserRole),
    NewClientConnectionAccepted(u64, SocketAddr),
    ClientConnectionAcceptFailed(Option<SocketAddr>, io::Error),
    ClientRequestedUnsupportedVersion(u64, u8),
    ClientRequestedUnsupportedCommand(u64, u8),
    ClientRequestedUnsupportedAtyp(u64, u8),
    ClientSelectedAuthMethod(u64, AuthMethod),
    ClientRequestedUnsupportedUserpassVersion(u64, u8),
    ClientAuthenticatedWithUserpass(u64, String, bool),
    ClientSocksRequest(u64, SocksRequest),
    ClientDnsLookup(u64, String),
    ClientAttemptingConnect(u64, SocketAddr),
    ClientConnectionAttemptBindFailed(u64, io::Error),
    ClientConnectionAttemptConnectFailed(u64, io::Error),
    ClientFailedToConnectToDestination(u64),
    ClientConnectedToDestination(u64, SocketAddr),
    ClientBytesSent(u64, u64),
    ClientBytesReceived(u64, u64),
    ClientSourceShutdown(u64),
    ClientDestinationShutdown(u64),
    ClientConnectionFinished(u64, u64, u64, Result<(), io::Error>),
}

impl fmt::Display for LogEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NewListeningSocket(socket_address) => write!(f, "Listening for client connections at {socket_address}"),
            Self::FailedBindListeningSocket(socket_address, io_error) => write!(f, "Failed to set up listening socket at {socket_address}: {io_error}"),
            Self::FailedBindAnySocketAborting => write!(f, "Failed to bind any socket! Aborting"),
            Self::RemovedListeningSocket(socket_address) => write!(f, "Will no longer listen for client connections at {socket_address}"),
            Self::LoadingUsersFromFile(filename) => write!(f, "Loading users from file {filename}"),
            Self::UsersLoadedFromFile(filename, Ok(user_count)) => write!(f, "Loaded {user_count} users from file {filename}"),
            Self::UsersLoadedFromFile(filename, Err(load_users_error)) => write!(f, "Error while loading users from file {filename}: {load_users_error}"),
            Self::StartingUpWithSingleDefaultUser => write!(f, "Starting up with single default user {DEFAULT_USER_USERNAME}:{DEFAULT_USER_PASSWORD}"),
            Self::SavingUsersToFile(filename) => write!(f, "Saving users to file {filename}"),
            Self::UsersSavedToFile(filename, Ok(amount)) => write!(f, "Successfully saved {amount} users to file {filename}"),
            Self::UsersSavedToFile(filename, Err(io_error)) => write!(f, "Failed to save users to file {filename}: {io_error}"),
            Self::UserRegistered(username, role) => write!(f, "Registered new {role} user {username}"),
            Self::UserReplacedByArgs(username, role) => write!(f, "Replaced user loaded from file {username} with new {role} user specified via argument"),
            Self::UserUpdated(username, role, password_updated) => {
                write!(f, "Updated user {username} with role {role}{}", if *password_updated {
                    " and new password"
                } else {
                    ", password unchanged"
                })
            },
            Self::UserDeleted(username, role) => write!(f, "Deleted {role} user {username}"),
            Self::NewClientConnectionAccepted(client_id, socket_address) => write!(f, "New client connection from {socket_address} assigned ID {client_id}"),
            Self::ClientConnectionAcceptFailed(Some(socket_address), io_error) => write!(f, "Failed to accept incoming connection from socket {socket_address}: {io_error}"),
            Self::ClientConnectionAcceptFailed(None, io_error) => write!(f, "Failed to accept incoming connection from unknown socket: {io_error}"),
            Self::ClientRequestedUnsupportedVersion(client_id, version) => write!(f, "Client {client_id} requested unsupported socks version: {version}"),
            Self::ClientRequestedUnsupportedCommand(client_id, command) => write!(f, "Client {client_id} requested unsupported socks command: {command}"),
            Self::ClientRequestedUnsupportedAtyp(client_id, atyp) => write!(f, "Client {client_id} requested unsupported socks ATYP: {atyp}"),
            Self::ClientSelectedAuthMethod(client_id, AuthMethod::NoAcceptableMethod) => write!(f, "Client {client_id} no acceptable authentication method found"),
            Self::ClientSelectedAuthMethod(client_id, auth_method) => write!(f, "Client {client_id} will use auth method {auth_method}"),
            Self::ClientRequestedUnsupportedUserpassVersion(client_id, version) => write!(f, "Client {client_id} requested unsupported userpass version: {version}"),
            Self::ClientAuthenticatedWithUserpass(client_id, username, true) => write!(f, "Client {client_id} successfully authenticated as user {username}"),
            Self::ClientAuthenticatedWithUserpass(client_id, username, false) => write!(f, "Client {client_id} unsuccessfully authenticated as user {username}"),
            Self::ClientSocksRequest(client_id, request) => {
                write!(f, "Client {client_id} requested to connect to ")?;
                match &request.destination {
                    SocksRequestAddress::IPv4(ipv4) => write!(f, "IPv4 {ipv4}:{}", request.port),
                    SocksRequestAddress::IPv6(ipv6) => write!(f, "IPv6 [{ipv6}]:{}", request.port),
                    SocksRequestAddress::Domainname(domainname) => write!(f, "domainname {domainname}:{}", request.port),
                }
            },
            Self::ClientDnsLookup(client_id, domainname) => write!(f, "Client {client_id} performing DNS lookup for {domainname}"),
            Self::ClientAttemptingConnect(client_id, socket_address) => write!(f, "Client {client_id} attempting to connect to destination at {socket_address}"),
            Self::ClientConnectionAttemptBindFailed(client_id, io_error) => write!(f, "Client {client_id} failed to bind local socket: {io_error}"),
            Self::ClientConnectionAttemptConnectFailed(client_id, io_error) => write!(f, "Client {client_id} failed to connect to destination: {io_error}"),
            Self::ClientFailedToConnectToDestination(client_id) => write!(f, "Client {client_id} failed to connect to destination, sending error response"),
            Self::ClientConnectedToDestination(client_id, socket_address) => write!(f, "Client {client_id} successfully established connection to destination at {socket_address}"),
            Self::ClientBytesSent(client_id, count) => write!(f, "Client {client_id} sent {count} bytes"),
            Self::ClientBytesReceived(client_id, count) => write!(f, "Client {client_id} received {count} bytes"),
            Self::ClientSourceShutdown(client_id) => write!(f, "Client {client_id} source socket shutdown"),
            Self::ClientDestinationShutdown(client_id) => write!(f, "Client {client_id} destination socket shutdown"),
            Self::ClientConnectionFinished(client_id, total_bytes_sent, total_bytes_received,Ok(())) => write!(f, "Client {client_id} finished after {total_bytes_sent} bytes sent and {total_bytes_received} bytes received"),
            Self::ClientConnectionFinished(client_id, total_bytes_sent, total_bytes_received, Err(io_error)) => write!(f, "Client {client_id} closed with IO error after {total_bytes_sent} bytes sent and {total_bytes_received} bytes received: {io_error}"),
        }
    }
}
