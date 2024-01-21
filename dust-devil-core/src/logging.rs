//! Defines the [`Event`] and [`EventData`] structs used for serializing events, as well as
//! implementing the [`fmt::Display`] trait for [`EventData`], which is used for turning events
//! into server logs.

use std::{
    fmt,
    io::{self, ErrorKind},
    net::SocketAddr,
};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    serialize::{ByteRead, ByteWrite, SmallReadString, SmallWriteString},
    socks5::{AuthMethod, SocksRequest, SocksRequestAddress},
    users::{UserRole, UsersLoadingError},
};

/// A server event, consisting of a UNIX timestamp and an [`EventData`] describing the event.
pub struct Event {
    pub timestamp: i64,
    pub data: EventData,
}

impl Event {
    pub fn new(timestamp: i64, data: EventData) -> Self {
        Event { timestamp, data }
    }
}

/// All the possible server events that can be reported.
pub enum EventData {
    NewSocks5Socket(SocketAddr),
    FailedBindSocks5Socket(SocketAddr, io::Error),
    FailedBindAnySocketAborting,
    RemovedSocks5Socket(SocketAddr),
    NewSandstormSocket(SocketAddr),
    FailedBindSandstormSocket(SocketAddr, io::Error),
    RemovedSandstormSocket(SocketAddr),
    LoadingUsersFromFile(String),
    UsersLoadedFromFile(String, Result<u64, UsersLoadingError>),
    StartingUpWithSingleDefaultUser(String),
    SavingUsersToFile(String),
    UsersSavedToFile(String, Result<u64, io::Error>),
    UserRegisteredByArgs(String, UserRole),
    UserReplacedByArgs(String, UserRole),
    NewClientConnectionAccepted(u64, SocketAddr),
    ClientConnectionAcceptFailed(Option<SocketAddr>, io::Error),
    ClientRequestedUnsupportedVersion(u64, u8),
    ClientRequestedUnsupportedCommand(u64, u8),
    ClientRequestedUnsupportedAtyp(u64, u8),
    ClientSelectedAuthMethod(u64, AuthMethod),
    ClientNoAcceptableAuthMethod(u64),
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
    NewSandstormConnectionAccepted(u64, SocketAddr),
    SandstormConnectionAcceptFailed(Option<SocketAddr>, io::Error),
    SandstormRequestedUnsupportedVersion(u64, u8),
    SandstormAuthenticatedAs(u64, String, bool),
    NewSocksSocketRequestedByManager(u64, SocketAddr),
    RemoveSocksSocketRequestedByManager(u64, SocketAddr),
    NewSandstormSocketRequestedByManager(u64, SocketAddr),
    RemoveSandstormSocketRequestedByManager(u64, SocketAddr),
    UserRegisteredByManager(u64, String, UserRole),
    UserUpdatedByManager(u64, String, UserRole, bool),
    UserDeletedByManager(u64, String, UserRole),
    AuthMethodToggledByManager(u64, AuthMethod, bool),
    BufferSizeChangedByManager(u64, u32),
    SandstormRequestedShutdown(u64),
    SandstormConnectionFinished(u64, Result<(), io::Error>),
    ShutdownSignalReceived,
}

impl fmt::Display for EventData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NewSocks5Socket(socket_address) => write!(f, "Listening for socks5 client connections at {socket_address}"),
            Self::FailedBindSocks5Socket(socket_address, io_error) => write!(f, "Failed to set up socks5 socket at {socket_address}: {io_error}"),
            Self::FailedBindAnySocketAborting => write!(f, "Failed to bind any socks5 socket! Aborting"),
            Self::RemovedSocks5Socket(socket_address) => write!(f, "Will no longer listen for socks5 client connections at {socket_address}"),
            Self::NewSandstormSocket(socket_address) => write!(f, "Listening for Sandstorm connections at {socket_address}"),
            Self::FailedBindSandstormSocket(socket_address, io_error) => write!(f, "Failed to set up Sandstorm socket at {socket_address}: {io_error}"),
            Self::RemovedSandstormSocket(socket_address) => write!(f, "Will no longer listen for Sandstorm connections at {socket_address}"),
            Self::LoadingUsersFromFile(filename) => write!(f, "Loading users from file {filename}"),
            Self::UsersLoadedFromFile(filename, Ok(user_count)) => write!(f, "Loaded {user_count} users from file {filename}"),
            Self::UsersLoadedFromFile(filename, Err(load_users_error)) => write!(f, "Error while loading users from file {filename}: {load_users_error}"),
            Self::StartingUpWithSingleDefaultUser(userpass) => write!(f, "Starting up with single default user {userpass}"),
            Self::SavingUsersToFile(filename) => write!(f, "Saving users to file {filename}"),
            Self::UsersSavedToFile(filename, Ok(amount)) => write!(f, "Successfully saved {amount} users to file {filename}"),
            Self::UsersSavedToFile(filename, Err(io_error)) => write!(f, "Failed to save users to file {filename}: {io_error}"),
            Self::UserRegisteredByArgs(username, role) => write!(f, "Registered new {role} user {username} specified via argument"),
            Self::UserReplacedByArgs(username, role) => write!(f, "Replaced user loaded from file {username} with new {role} user specified via argument"),
            Self::NewClientConnectionAccepted(client_id, socket_address) => write!(f, "New client connection from {socket_address} assigned ID {client_id}"),
            Self::ClientConnectionAcceptFailed(Some(socket_address), io_error) => write!(f, "Failed to accept incoming socks connection from socket {socket_address}: {io_error}"),
            Self::ClientConnectionAcceptFailed(None, io_error) => write!(f, "Failed to accept incoming socks connection from unknown socket: {io_error}"),
            Self::ClientRequestedUnsupportedVersion(client_id, version) => write!(f, "Client {client_id} requested unsupported socks version: {version}"),
            Self::ClientRequestedUnsupportedCommand(client_id, command) => write!(f, "Client {client_id} requested unsupported socks command: {command}"),
            Self::ClientRequestedUnsupportedAtyp(client_id, atyp) => write!(f, "Client {client_id} requested unsupported socks ATYP: {atyp}"),
            Self::ClientSelectedAuthMethod(client_id, auth_method) => write!(f, "Client {client_id} will use auth method {auth_method}"),
            Self::ClientNoAcceptableAuthMethod(client_id) => write!(f, "Client {client_id} no acceptable authentication method found"),
            Self::ClientRequestedUnsupportedUserpassVersion(client_id, version) => write!(f, "Client {client_id} requested unsupported userpass version: {version}"),
            Self::ClientAuthenticatedWithUserpass(client_id, username, true) => write!(f, "Client {client_id} successfully authenticated as {username}"),
            Self::ClientAuthenticatedWithUserpass(client_id, username, false) => write!(f, "Client {client_id} unsuccessfully authenticated as {username}"),
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
            Self::NewSandstormConnectionAccepted(manager_id, socket_address) => write!(f, "New management connection from {socket_address} assigned ID {manager_id}"),
            Self::SandstormConnectionAcceptFailed(Some(socket_address), io_error) => write!(f, "Failed to accept incoming management connection from socket {socket_address}: {io_error}"),
            Self::SandstormConnectionAcceptFailed(None, io_error) => write!(f, "Failed to accept incoming management connection from unknown socket: {io_error}"),
            Self::SandstormRequestedUnsupportedVersion(manager_id, version) => write!(f, "Manager {manager_id} requested unsupported sandstorm version: {version}"),
            Self::SandstormAuthenticatedAs(manager_id, username, true) => write!(f, "Manager {manager_id} successfully authenticated as {username}"),
            Self::SandstormAuthenticatedAs(manager_id, username, false) => write!(f, "Manager {manager_id} unsuccessfully authenticated as {username}"),
            Self::NewSocksSocketRequestedByManager(manager_id, socket_address) => write!(f, "Manager {manager_id} requested opening a new socks5 socket at {socket_address}"),
            Self::RemoveSocksSocketRequestedByManager(manager_id, socket_address) => write!(f, "Manager {manager_id} requested closing socks5 socket at {socket_address}"),
            Self::NewSandstormSocketRequestedByManager(manager_id, socket_address) => write!(f, "Manager {manager_id} requested opening a new sandstorm socket at {socket_address}"),
            Self::RemoveSandstormSocketRequestedByManager(manager_id, socket_address) => write!(f, "Manager {manager_id} requested closing sandstorm socket at {socket_address}"),
            Self::UserRegisteredByManager(manager_id, username, role) => write!(f, "Manager {manager_id} registered new {role} user {username}"),
            Self::UserUpdatedByManager(manager_id, username, role, false) => write!(f, "Manager {manager_id} updated role of user {username} to {role}"),
            Self::UserUpdatedByManager(manager_id, username, role, true) => write!(f, "Manager {manager_id} updated user {username} with role {role} and new password"),
            Self::UserDeletedByManager(manager_id, username, role) => write!(f, "Manager {manager_id} deleted {role} user {username}"),
            Self::AuthMethodToggledByManager(manager_id, auth_method, false) => write!(f, "Manager {manager_id} disabled authentication method {auth_method}"),
            Self::AuthMethodToggledByManager(manager_id, auth_method, true) => write!(f, "Manager {manager_id} enabled authentication method {auth_method}"),
            Self::BufferSizeChangedByManager(manager_id, buffer_size) => write!(f, "Manager {manager_id} set client buffer size to {buffer_size}"),
            Self::SandstormRequestedShutdown(manager_id) => write!(f, "Manager {manager_id} requested the server shuts down"),
            Self::SandstormConnectionFinished(manager_id, Ok(())) => write!(f, "Manager {manager_id} finished"),
            Self::SandstormConnectionFinished(manager_id, Err(io_error)) => write!(f, "Manager {manager_id} closed with IO error: {io_error}"),
            Self::ShutdownSignalReceived => write!(f, "Shutdown signal received"),
        }
    }
}

impl ByteRead for EventData {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let t = u8::read(reader).await?;

        match t {
            0x01 => Ok(Self::NewSocks5Socket(SocketAddr::read(reader).await?)),
            0x02 => Ok(Self::FailedBindSocks5Socket(
                SocketAddr::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x03 => Ok(Self::FailedBindAnySocketAborting),
            0x04 => Ok(Self::RemovedSocks5Socket(SocketAddr::read(reader).await?)),
            0x05 => Ok(Self::NewSandstormSocket(SocketAddr::read(reader).await?)),
            0x06 => Ok(Self::FailedBindSandstormSocket(
                SocketAddr::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x07 => Ok(Self::RemovedSandstormSocket(SocketAddr::read(reader).await?)),
            0x08 => Ok(Self::LoadingUsersFromFile(String::read(reader).await?)),
            0x09 => Ok(Self::UsersLoadedFromFile(
                String::read(reader).await?,
                <Result<u64, UsersLoadingError> as ByteRead>::read(reader).await?,
            )),
            0x0A => Ok(Self::StartingUpWithSingleDefaultUser(String::read(reader).await?)),
            0x0B => Ok(Self::SavingUsersToFile(String::read(reader).await?)),
            0x0C => Ok(Self::UsersSavedToFile(
                String::read(reader).await?,
                <Result<u64, io::Error> as ByteRead>::read(reader).await?,
            )),
            0x0D => Ok(Self::UserRegisteredByArgs(
                SmallReadString::read(reader).await?.0,
                UserRole::read(reader).await?,
            )),
            0x0E => Ok(Self::UserReplacedByArgs(
                SmallReadString::read(reader).await?.0,
                UserRole::read(reader).await?,
            )),
            0x0F => Ok(Self::NewClientConnectionAccepted(
                u64::read(reader).await?,
                SocketAddr::read(reader).await?,
            )),
            0x10 => Ok(Self::ClientConnectionAcceptFailed(
                <Option<SocketAddr> as ByteRead>::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x11 => Ok(Self::ClientRequestedUnsupportedVersion(
                u64::read(reader).await?,
                u8::read(reader).await?,
            )),
            0x12 => Ok(Self::ClientRequestedUnsupportedCommand(
                u64::read(reader).await?,
                u8::read(reader).await?,
            )),
            0x13 => Ok(Self::ClientRequestedUnsupportedAtyp(
                u64::read(reader).await?,
                u8::read(reader).await?,
            )),
            0x14 => Ok(Self::ClientSelectedAuthMethod(
                u64::read(reader).await?,
                AuthMethod::read(reader).await?,
            )),
            0x15 => Ok(Self::ClientNoAcceptableAuthMethod(u64::read(reader).await?)),
            0x16 => Ok(Self::ClientRequestedUnsupportedUserpassVersion(
                u64::read(reader).await?,
                u8::read(reader).await?,
            )),
            0x17 => Ok(Self::ClientAuthenticatedWithUserpass(
                u64::read(reader).await?,
                String::read(reader).await?,
                bool::read(reader).await?,
            )),
            0x18 => Ok(Self::ClientSocksRequest(
                u64::read(reader).await?,
                SocksRequest::read(reader).await?,
            )),
            0x19 => Ok(Self::ClientDnsLookup(
                u64::read(reader).await?,
                SmallReadString::read(reader).await?.0,
            )),
            0x1A => Ok(Self::ClientAttemptingConnect(
                u64::read(reader).await?,
                SocketAddr::read(reader).await?,
            )),
            0x1B => Ok(Self::ClientConnectionAttemptBindFailed(
                u64::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x1C => Ok(Self::ClientConnectionAttemptConnectFailed(
                u64::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x1D => Ok(Self::ClientFailedToConnectToDestination(u64::read(reader).await?)),
            0x1E => Ok(Self::ClientConnectedToDestination(
                u64::read(reader).await?,
                SocketAddr::read(reader).await?,
            )),
            0x1F => Ok(Self::ClientBytesSent(u64::read(reader).await?, u64::read(reader).await?)),
            0x20 => Ok(Self::ClientBytesReceived(u64::read(reader).await?, u64::read(reader).await?)),
            0x21 => Ok(Self::ClientSourceShutdown(u64::read(reader).await?)),
            0x22 => Ok(Self::ClientDestinationShutdown(u64::read(reader).await?)),
            0x23 => Ok(Self::ClientConnectionFinished(
                u64::read(reader).await?,
                u64::read(reader).await?,
                u64::read(reader).await?,
                <Result<(), io::Error> as ByteRead>::read(reader).await?,
            )),
            0x24 => Ok(Self::NewSandstormConnectionAccepted(
                u64::read(reader).await?,
                SocketAddr::read(reader).await?,
            )),
            0x25 => Ok(Self::SandstormConnectionAcceptFailed(
                <Option<SocketAddr> as ByteRead>::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x26 => Ok(Self::SandstormRequestedUnsupportedVersion(
                u64::read(reader).await?,
                u8::read(reader).await?,
            )),
            0x27 => Ok(Self::SandstormAuthenticatedAs(
                u64::read(reader).await?,
                String::read(reader).await?,
                bool::read(reader).await?,
            )),
            0x28 => Ok(Self::NewSocksSocketRequestedByManager(
                u64::read(reader).await?,
                SocketAddr::read(reader).await?,
            )),
            0x29 => Ok(Self::RemoveSocksSocketRequestedByManager(
                u64::read(reader).await?,
                SocketAddr::read(reader).await?,
            )),
            0x2A => Ok(Self::NewSandstormSocketRequestedByManager(
                u64::read(reader).await?,
                SocketAddr::read(reader).await?,
            )),
            0x2B => Ok(Self::RemoveSandstormSocketRequestedByManager(
                u64::read(reader).await?,
                SocketAddr::read(reader).await?,
            )),
            0x2C => Ok(Self::UserRegisteredByManager(
                u64::read(reader).await?,
                String::read(reader).await?,
                UserRole::read(reader).await?,
            )),
            0x2D => Ok(Self::UserUpdatedByManager(
                u64::read(reader).await?,
                String::read(reader).await?,
                UserRole::read(reader).await?,
                bool::read(reader).await?,
            )),
            0x2E => Ok(Self::UserDeletedByManager(
                u64::read(reader).await?,
                String::read(reader).await?,
                UserRole::read(reader).await?,
            )),
            0x2F => Ok(Self::AuthMethodToggledByManager(
                u64::read(reader).await?,
                AuthMethod::read(reader).await?,
                bool::read(reader).await?,
            )),
            0x30 => Ok(Self::BufferSizeChangedByManager(u64::read(reader).await?, u32::read(reader).await?)),
            0x31 => Ok(Self::SandstormRequestedShutdown(u64::read(reader).await?)),
            0x32 => Ok(Self::SandstormConnectionFinished(
                u64::read(reader).await?,
                <Result<(), io::Error> as ByteRead>::read(reader).await?,
            )),
            0x33 => Ok(Self::ShutdownSignalReceived),
            _ => Err(io::Error::new(ErrorKind::InvalidData, "Invalid EventData type byte")),
        }
    }
}

impl ByteWrite for EventData {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Self::NewSocks5Socket(socket_address) => (0x01u8, socket_address).write(writer).await,
            Self::FailedBindSocks5Socket(socket_address, io_error) => (0x02u8, socket_address, io_error).write(writer).await,
            Self::FailedBindAnySocketAborting => 0x03u8.write(writer).await,
            Self::RemovedSocks5Socket(socket_address) => (0x04u8, socket_address).write(writer).await,
            Self::NewSandstormSocket(socket_address) => (0x05u8, socket_address).write(writer).await,
            Self::FailedBindSandstormSocket(socket_address, io_error) => (0x06u8, socket_address, io_error).write(writer).await,
            Self::RemovedSandstormSocket(socket_address) => (0x07u8, socket_address).write(writer).await,
            Self::LoadingUsersFromFile(filename) => (0x08u8, filename).write(writer).await,
            Self::UsersLoadedFromFile(filename, result) => (0x09u8, filename, result).write(writer).await,
            Self::StartingUpWithSingleDefaultUser(userpass) => (0x0Au8, userpass).write(writer).await,
            Self::SavingUsersToFile(filename) => (0x0Bu8, filename).write(writer).await,
            Self::UsersSavedToFile(filename, result) => (0x0Cu8, filename, result).write(writer).await,
            Self::UserRegisteredByArgs(username, role) => (0x0Du8, SmallWriteString(username), role).write(writer).await,
            Self::UserReplacedByArgs(username, role) => (0x0Eu8, SmallWriteString(username), role).write(writer).await,
            Self::NewClientConnectionAccepted(client_id, socket_address) => (0x0Fu8, client_id, socket_address).write(writer).await,
            Self::ClientConnectionAcceptFailed(maybe_socket_address, io_error) => {
                (0x10u8, maybe_socket_address, io_error).write(writer).await
            }
            Self::ClientRequestedUnsupportedVersion(client_id, ver) => (0x11u8, client_id, ver).write(writer).await,
            Self::ClientRequestedUnsupportedCommand(client_id, cmd) => (0x12u8, client_id, cmd).write(writer).await,
            Self::ClientRequestedUnsupportedAtyp(client_id, atyp) => (0x13u8, client_id, atyp).write(writer).await,
            Self::ClientSelectedAuthMethod(client_id, auth_method) => (0x14u8, client_id, auth_method).write(writer).await,
            Self::ClientNoAcceptableAuthMethod(client_id) => (0x15u8, client_id).write(writer).await,
            Self::ClientRequestedUnsupportedUserpassVersion(client_id, ver) => (0x16u8, client_id, ver).write(writer).await,
            Self::ClientAuthenticatedWithUserpass(client_id, username, success) => {
                (0x17u8, client_id, SmallWriteString(username), success).write(writer).await
            }
            Self::ClientSocksRequest(client_id, request) => (0x18u8, client_id, request).write(writer).await,
            Self::ClientDnsLookup(client_id, domainname) => (0x19u8, client_id, SmallWriteString(domainname)).write(writer).await,
            Self::ClientAttemptingConnect(client_id, socket_address) => (0x1Au8, client_id, socket_address).write(writer).await,
            Self::ClientConnectionAttemptBindFailed(client_id, io_error) => (0x1Bu8, client_id, io_error).write(writer).await,
            Self::ClientConnectionAttemptConnectFailed(client_id, io_error) => (0x1Cu8, client_id, io_error).write(writer).await,
            Self::ClientFailedToConnectToDestination(client_id) => (0x1Du8, client_id).write(writer).await,
            Self::ClientConnectedToDestination(client_id, socket_address) => (0x1Eu8, client_id, socket_address).write(writer).await,
            Self::ClientBytesSent(client_id, count) => (0x1Fu8, client_id, count).write(writer).await,
            Self::ClientBytesReceived(client_id, count) => (0x20u8, client_id, count).write(writer).await,
            Self::ClientSourceShutdown(client_id) => (0x21u8, client_id).write(writer).await,
            Self::ClientDestinationShutdown(client_id) => (0x22u8, client_id).write(writer).await,
            Self::ClientConnectionFinished(client_id, sent, received, result) => {
                (0x23u8, client_id, sent, received, result).write(writer).await
            }
            Self::NewSandstormConnectionAccepted(manager_id, socket_address) => (0x24u8, manager_id, socket_address).write(writer).await,
            Self::SandstormConnectionAcceptFailed(maybe_socket_address, io_error) => {
                (0x25u8, maybe_socket_address, io_error).write(writer).await
            }
            Self::SandstormRequestedUnsupportedVersion(manager_id, version) => (0x26u8, manager_id, version).write(writer).await,
            Self::SandstormAuthenticatedAs(manager_id, username, success) => (0x27u8, manager_id, username, success).write(writer).await,
            Self::NewSocksSocketRequestedByManager(manager_id, socket_address) => (0x28u8, manager_id, socket_address).write(writer).await,
            Self::RemoveSocksSocketRequestedByManager(manager_id, socket_address) => {
                (0x29u8, manager_id, socket_address).write(writer).await
            }
            Self::NewSandstormSocketRequestedByManager(manager_id, socket_address) => {
                (0x2Au8, manager_id, socket_address).write(writer).await
            }
            Self::RemoveSandstormSocketRequestedByManager(manager_id, socket_address) => {
                (0x2Bu8, manager_id, socket_address).write(writer).await
            }
            Self::UserRegisteredByManager(manager_id, username, role) => (0x2Cu8, manager_id, username, role).write(writer).await,
            Self::UserUpdatedByManager(manager_id, username, role, password_changed) => {
                (0x2Du8, manager_id, username, role, password_changed).write(writer).await
            }
            Self::UserDeletedByManager(manager_id, username, role) => (0x2Eu8, manager_id, username, role).write(writer).await,
            Self::AuthMethodToggledByManager(manager_id, auth_method, enabled) => {
                (0x2Fu8, manager_id, auth_method, enabled).write(writer).await
            }
            Self::BufferSizeChangedByManager(manager_id, buffer_size) => (0x30u8, manager_id, buffer_size).write(writer).await,
            Self::SandstormRequestedShutdown(manager_id) => (0x31u8, manager_id).write(writer).await,
            Self::SandstormConnectionFinished(manager_id, result) => (0x32u8, manager_id, result).write(writer).await,
            Self::ShutdownSignalReceived => 0x33u8.write(writer).await,
        }
    }
}

impl ByteRead for Event {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self::new(i64::read(reader).await?, EventData::read(reader).await?))
    }
}

impl ByteWrite for Event {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.timestamp.write(writer).await?;
        self.data.write(writer).await
    }
}
