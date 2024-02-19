use dust_devil_core::{logging, socks5::SocksRequestAddress};
use ratatui::style::{Color, Modifier, Style};
use time::{OffsetDateTime, UtcOffset};

use crate::tui::text_wrapper::StaticString;

pub const BOLD_ITALIC: Style = Style::reset().add_modifier(Modifier::BOLD).add_modifier(Modifier::ITALIC);
pub const DEFAULT_STYLE: Style = Style::reset();
pub const TIMESTAMP_STYLE: Style = Style::reset().fg(Color::DarkGray);
pub const SOCKET_ADDRESS_STYLE: Style = Style::reset().fg(Color::LightYellow);
pub const FILENAME_STYLE: Style = Style::reset().fg(Color::LightCyan);
pub const USERNAME_STYLE: Style = Style::reset().fg(Color::LightBlue);
pub const USERROLE_STYLE: Style = USERNAME_STYLE;
pub const AUTH_METHOD_STYLE: Style = USERNAME_STYLE;
pub const CLIENT_ID_STYLE: Style = Style::reset().fg(Color::LightGreen);
pub const CLIENT_ADDRESS_STYLE: Style = SOCKET_ADDRESS_STYLE;
pub const DESTINATION_ADDRESS_STYLE: Style = SOCKET_ADDRESS_STYLE;
pub const MANAGER_ID_STYLE: Style = Style::reset().fg(Color::LightMagenta);
pub const MANAGER_ADDRESS_STYLE: Style = SOCKET_ADDRESS_STYLE;
pub const BUFFER_SIZE_STYLE: Style = Style::reset().fg(Color::LightRed);
pub const SHUTDOWN_REQUEST_STYLE: Style = BOLD_ITALIC.fg(Color::Red);
pub const WARNING_STYLE: Style = Style::reset().fg(Color::Yellow);
pub const ERROR_STYLE: Style = Style::reset().fg(Color::Red);
pub const SHUTDOWN_SIGNAL_STYLE: Style = BOLD_ITALIC.fg(Color::Red);

pub fn log_event_to_single_line(vec: &mut Vec<(StaticString, Style)>, utc_offset: UtcOffset, event: &logging::Event) {
    let t = OffsetDateTime::from_unix_timestamp(event.timestamp)
        .map(|t| t.to_offset(utc_offset))
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);

    vec.push((
        format!(
            "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}]",
            t.year(),
            t.month() as u8,
            t.day(),
            t.hour(),
            t.minute(),
            t.second(),
        )
        .into(),
        TIMESTAMP_STYLE,
    ));

    match &event.data {
        logging::EventData::NewSocks5Socket(socket_address) => {
            vec.push((" Listening for socks5 client connections at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
        }
        logging::EventData::FailedBindSocks5Socket(socket_address, io_error) => {
            vec.push((" Failed to set up socks5 socket at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
            vec.push((": ".into(), DEFAULT_STYLE));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::FailedBindAnySocketAborting => {
            vec.push((" Failed to bind any socks5 socket! Aborting".into(), ERROR_STYLE));
        }
        logging::EventData::RemovedSocks5Socket(socket_address) => {
            vec.push((" Will no longer listen for socks5 client connections at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
        }
        logging::EventData::NewSandstormSocket(socket_address) => {
            vec.push((" Listening for Sandstorm connections at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
        }
        logging::EventData::FailedBindSandstormSocket(socket_address, io_error) => {
            vec.push((" Failed to set up Sandstorm socket at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
            vec.push((": ".into(), DEFAULT_STYLE));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::RemovedSandstormSocket(socket_address) => {
            vec.push((" Will no longer listen for Sandstorm connections at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
        }
        logging::EventData::LoadingUsersFromFile(filename) => {
            vec.push((" Loading users from file ".into(), DEFAULT_STYLE));
            vec.push((filename.clone().into(), FILENAME_STYLE));
        }
        logging::EventData::UsersLoadedFromFile(filename, Ok(user_count)) => {
            vec.push((" Loaded ".into(), DEFAULT_STYLE));
            vec.push((format!("{user_count}").into(), DEFAULT_STYLE));
            vec.push((" users from file ".into(), DEFAULT_STYLE));
            vec.push((filename.clone().into(), FILENAME_STYLE));
        }
        logging::EventData::UsersLoadedFromFile(filename, Err(load_users_error)) => {
            vec.push((" Error while loading users from file ".into(), DEFAULT_STYLE));
            vec.push((filename.clone().into(), FILENAME_STYLE));
            vec.push((": ".into(), DEFAULT_STYLE));
            vec.push((format!("{load_users_error}").into(), ERROR_STYLE));
        }
        logging::EventData::StartingUpWithSingleDefaultUser(userpass) => {
            vec.push((" Starting up with single default user ".into(), DEFAULT_STYLE));
            vec.push((userpass.clone().into(), USERNAME_STYLE));
        }
        logging::EventData::SavingUsersToFile(filename) => {
            vec.push((" Saving users to file ".into(), DEFAULT_STYLE));
            vec.push((filename.clone().into(), FILENAME_STYLE));
        }
        logging::EventData::UsersSavedToFile(filename, Ok(amount)) => {
            vec.push((" Successfully saved ".into(), DEFAULT_STYLE));
            vec.push((format!("{amount}").into(), DEFAULT_STYLE));
            vec.push((" users to file ".into(), DEFAULT_STYLE));
            vec.push((filename.clone().into(), FILENAME_STYLE));
        }
        logging::EventData::UsersSavedToFile(filename, Err(io_error)) => {
            vec.push((" Failed to save users to file ".into(), DEFAULT_STYLE));
            vec.push((filename.clone().into(), FILENAME_STYLE));
            vec.push((": ".into(), DEFAULT_STYLE));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::UserRegisteredByArgs(username, role) => {
            vec.push((" Registered new ".into(), DEFAULT_STYLE));
            vec.push((role.to_str().into(), USERROLE_STYLE));
            vec.push((" user ".into(), DEFAULT_STYLE));
            vec.push((username.clone().into(), USERNAME_STYLE));
            vec.push((" specified via argument".into(), DEFAULT_STYLE));
        }
        logging::EventData::UserReplacedByArgs(username, role) => {
            vec.push((" Replaced user loaded from file ".into(), DEFAULT_STYLE));
            vec.push((username.clone().into(), USERNAME_STYLE));
            vec.push((" with new ".into(), DEFAULT_STYLE));
            vec.push((role.to_str().into(), USERROLE_STYLE));
            vec.push((" user specified via argument".into(), DEFAULT_STYLE));
        }
        logging::EventData::NewClientConnectionAccepted(client_id, socket_address) => {
            vec.push((" New client connection from ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), CLIENT_ADDRESS_STYLE));
            vec.push((" assigned ".into(), DEFAULT_STYLE));
            vec.push((format!("ID {client_id}").into(), CLIENT_ID_STYLE));
        }
        logging::EventData::ClientConnectionAcceptFailed(Some(socket_address), io_error) => {
            vec.push((" Failed to accept incoming socks connection from socket ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), CLIENT_ADDRESS_STYLE));
            vec.push((": ".into(), DEFAULT_STYLE));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::ClientConnectionAcceptFailed(None, io_error) => {
            vec.push((
                " Failed to accept incoming socks connection from unknown socket: ".into(),
                DEFAULT_STYLE,
            ));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::ClientRequestedUnsupportedVersion(client_id, version) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" requested ".into(), DEFAULT_STYLE));
            vec.push(("unsupported socks version: ".into(), WARNING_STYLE));
            vec.push((format!("{version}").into(), WARNING_STYLE));
        }
        logging::EventData::ClientRequestedUnsupportedCommand(client_id, command) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" requested ".into(), DEFAULT_STYLE));
            vec.push(("unsupported socks command: ".into(), WARNING_STYLE));
            vec.push((format!("{command}").into(), WARNING_STYLE));
        }
        logging::EventData::ClientRequestedUnsupportedAtyp(client_id, atyp) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" requested ".into(), DEFAULT_STYLE));
            vec.push(("unsupported socks ATYP: ".into(), WARNING_STYLE));
            vec.push((format!("{atyp}").into(), WARNING_STYLE));
        }
        logging::EventData::ClientSelectedAuthMethod(client_id, auth_method) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" will use auth method ".into(), DEFAULT_STYLE));
            vec.push((auth_method.to_str().into(), AUTH_METHOD_STYLE));
        }
        logging::EventData::ClientNoAcceptableAuthMethod(client_id) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" no acceptable authentication method found".into(), WARNING_STYLE));
        }
        logging::EventData::ClientRequestedUnsupportedUserpassVersion(client_id, version) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" requested ".into(), DEFAULT_STYLE));
            vec.push(("unsupported userpass version: ".into(), WARNING_STYLE));
            vec.push((format!("{version}").into(), WARNING_STYLE));
        }
        logging::EventData::ClientAuthenticatedWithUserpass(client_id, username, true) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" successfully authenticated as ".into(), DEFAULT_STYLE));
            vec.push((username.clone().into(), USERNAME_STYLE));
        }
        logging::EventData::ClientAuthenticatedWithUserpass(client_id, username, false) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" unsuccessfully authenticated as ".into(), WARNING_STYLE));
            vec.push((username.clone().into(), USERNAME_STYLE));
        }
        logging::EventData::ClientSocksRequest(client_id, request) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" requested to connect to ".into(), DEFAULT_STYLE));
            match &request.destination {
                SocksRequestAddress::IPv4(ipv4) => {
                    vec.push(("IPv4 ".into(), DEFAULT_STYLE));
                    vec.push((format!("{ipv4}:{}", request.port).into(), DESTINATION_ADDRESS_STYLE));
                }
                SocksRequestAddress::IPv6(ipv6) => {
                    vec.push(("IPv6 ".into(), DEFAULT_STYLE));
                    vec.push((format!("[{ipv6}]:{}", request.port).into(), DESTINATION_ADDRESS_STYLE));
                }
                SocksRequestAddress::Domainname(domainname) => {
                    vec.push(("domainname ".into(), DEFAULT_STYLE));
                    vec.push((format!("{domainname}:{}", request.port).into(), DESTINATION_ADDRESS_STYLE));
                }
            }
        }
        logging::EventData::ClientDnsLookup(client_id, domainname) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" performing DNS lookup for ".into(), DEFAULT_STYLE));
            vec.push((domainname.clone().into(), DESTINATION_ADDRESS_STYLE));
        }
        logging::EventData::ClientAttemptingConnect(client_id, socket_address) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" attempting to connect to destination at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), DESTINATION_ADDRESS_STYLE));
        }
        logging::EventData::ClientConnectionAttemptBindFailed(client_id, io_error) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" failed to bind local socket: ".into(), DEFAULT_STYLE));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::ClientConnectionAttemptConnectFailed(client_id, io_error) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" failed to connect to destination: ".into(), DEFAULT_STYLE));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::ClientFailedToConnectToDestination(client_id) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" failed to connect to destination, ".into(), DEFAULT_STYLE));
            vec.push(("sending error response".into(), ERROR_STYLE));
        }
        logging::EventData::ClientConnectedToDestination(client_id, socket_address) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" successfully established connection to destination at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), DESTINATION_ADDRESS_STYLE));
        }
        logging::EventData::ClientBytesSent(client_id, count) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" sent ".into(), DEFAULT_STYLE));
            vec.push((format!("{count}").into(), DEFAULT_STYLE));
            vec.push((" bytes".into(), DEFAULT_STYLE));
        }
        logging::EventData::ClientBytesReceived(client_id, count) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" received ".into(), DEFAULT_STYLE));
            vec.push((format!("{count}").into(), DEFAULT_STYLE));
            vec.push((" bytes".into(), DEFAULT_STYLE));
        }
        logging::EventData::ClientSourceShutdown(client_id) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" source socket shutdown".into(), DEFAULT_STYLE));
        }
        logging::EventData::ClientDestinationShutdown(client_id) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" destination socket shutdown".into(), DEFAULT_STYLE));
        }
        logging::EventData::ClientConnectionFinished(client_id, total_bytes_sent, total_bytes_received, Ok(())) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" finished after ".into(), DEFAULT_STYLE));
            vec.push((format!("{total_bytes_sent}").into(), DEFAULT_STYLE));
            vec.push((" bytes sent and ".into(), DEFAULT_STYLE));
            vec.push((format!("{total_bytes_received}").into(), DEFAULT_STYLE));
            vec.push((" bytes received".into(), DEFAULT_STYLE));
        }
        logging::EventData::ClientConnectionFinished(client_id, total_bytes_sent, total_bytes_received, Err(io_error)) => {
            vec.push((" Client ".into(), CLIENT_ID_STYLE));
            vec.push((format!("{client_id}").into(), CLIENT_ID_STYLE));
            vec.push((" closed with IO error after ".into(), DEFAULT_STYLE));
            vec.push((format!("{total_bytes_sent}").into(), DEFAULT_STYLE));
            vec.push((" bytes sent and ".into(), DEFAULT_STYLE));
            vec.push((format!("{total_bytes_received}").into(), DEFAULT_STYLE));
            vec.push((" bytes received: ".into(), DEFAULT_STYLE));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::NewSandstormConnectionAccepted(manager_id, socket_address) => {
            vec.push((" New management connection from ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), MANAGER_ADDRESS_STYLE));
            vec.push((" assigned ".into(), DEFAULT_STYLE));
            vec.push((format!("ID {manager_id}").into(), MANAGER_ID_STYLE));
        }
        logging::EventData::SandstormConnectionAcceptFailed(Some(socket_address), io_error) => {
            vec.push((
                " Failed to accept incoming management connection from socket ".into(),
                DEFAULT_STYLE,
            ));
            vec.push((format!("{socket_address}").into(), MANAGER_ADDRESS_STYLE));
            vec.push((": ".into(), DEFAULT_STYLE));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::SandstormConnectionAcceptFailed(None, io_error) => {
            vec.push((
                " Failed to accept incoming management connection from unknown socket: ".into(),
                DEFAULT_STYLE,
            ));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::SandstormRequestedUnsupportedVersion(manager_id, version) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" requested ".into(), DEFAULT_STYLE));
            vec.push(("unsupported sandstorm version: ".into(), WARNING_STYLE));
            vec.push((format!("{version}").into(), WARNING_STYLE));
        }
        logging::EventData::SandstormAuthenticatedAs(manager_id, username, true) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" successfully authenticated as ".into(), DEFAULT_STYLE));
            vec.push((username.clone().into(), USERNAME_STYLE));
        }
        logging::EventData::SandstormAuthenticatedAs(manager_id, username, false) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" unsuccessfully authenticated as ".into(), WARNING_STYLE));
            vec.push((username.clone().into(), DEFAULT_STYLE));
        }
        logging::EventData::NewSocksSocketRequestedByManager(manager_id, socket_address) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" requested opening a new socks5 socket at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
        }
        logging::EventData::RemoveSocksSocketRequestedByManager(manager_id, socket_address) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" requested closing socks5 socket at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
        }
        logging::EventData::NewSandstormSocketRequestedByManager(manager_id, socket_address) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" requested opening a new sandstorm socket at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
        }
        logging::EventData::RemoveSandstormSocketRequestedByManager(manager_id, socket_address) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" requested closing sandstorm socket at ".into(), DEFAULT_STYLE));
            vec.push((format!("{socket_address}").into(), SOCKET_ADDRESS_STYLE));
        }
        logging::EventData::UserRegisteredByManager(manager_id, username, role) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" registered new ".into(), DEFAULT_STYLE));
            vec.push((role.to_str().into(), USERROLE_STYLE));
            vec.push((" user ".into(), DEFAULT_STYLE));
            vec.push((username.clone().into(), USERNAME_STYLE));
        }
        logging::EventData::UserUpdatedByManager(manager_id, username, role, false) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" updated role of user ".into(), DEFAULT_STYLE));
            vec.push((username.clone().into(), USERNAME_STYLE));
            vec.push((" to ".into(), DEFAULT_STYLE));
            vec.push((role.to_str().into(), USERROLE_STYLE));
        }
        logging::EventData::UserUpdatedByManager(manager_id, username, role, true) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" updated user ".into(), DEFAULT_STYLE));
            vec.push((username.clone().into(), USERNAME_STYLE));
            vec.push((" with role ".into(), DEFAULT_STYLE));
            vec.push((role.to_str().into(), USERROLE_STYLE));
            vec.push((" and new password".into(), DEFAULT_STYLE));
        }
        logging::EventData::UserDeletedByManager(manager_id, username, role) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" deleted ".into(), DEFAULT_STYLE));
            vec.push((role.to_str().into(), USERROLE_STYLE));
            vec.push((" user ".into(), DEFAULT_STYLE));
            vec.push((username.clone().into(), USERNAME_STYLE));
        }
        logging::EventData::AuthMethodToggledByManager(manager_id, auth_method, enabled) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push(((if *enabled { " enabled" } else { " disabled" }).into(), DEFAULT_STYLE));
            vec.push((" authentication method ".into(), DEFAULT_STYLE));
            vec.push((auth_method.to_str().into(), AUTH_METHOD_STYLE));
        }
        logging::EventData::BufferSizeChangedByManager(manager_id, buffer_size) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" set client buffer size to ".into(), DEFAULT_STYLE));
            vec.push((format!("{buffer_size}").into(), BUFFER_SIZE_STYLE));
        }
        logging::EventData::SandstormRequestedShutdown(manager_id) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" requested the server shuts down".into(), SHUTDOWN_REQUEST_STYLE));
        }
        logging::EventData::SandstormConnectionFinished(manager_id, Ok(())) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" finished".into(), DEFAULT_STYLE));
        }
        logging::EventData::SandstormConnectionFinished(manager_id, Err(io_error)) => {
            vec.push((" Manager ".into(), MANAGER_ID_STYLE));
            vec.push((format!("{manager_id}").into(), MANAGER_ID_STYLE));
            vec.push((" closed with IO error: ".into(), DEFAULT_STYLE));
            vec.push((format!("{io_error}").into(), ERROR_STYLE));
        }
        logging::EventData::ShutdownSignalReceived => {
            vec.push((" Shutdown signal received".into(), SHUTDOWN_SIGNAL_STYLE));
        }
    }
}
