use std::{
    collections::HashMap,
    fmt,
    io::ErrorKind,
    net::{Ipv6Addr, SocketAddr, SocketAddrV6, ToSocketAddrs},
};

use crate::users::{self, UserData};

const DEFAULT_PORT: u16 = 1080;
const DEFAULT_USERS_FILE: &str = "users.txt";

pub fn get_version_string() -> String {
    format!(
        concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), " ({} {})"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub fn get_help_string() -> &'static str {
    // TODO: Write a help menu
    "Help? I need somebody."
}

#[derive(Debug)]
pub struct StartupArguments {
    pub socks5_bind_sockets: Vec<SocketAddr>,
    pub verbose: bool,
    pub users_file: String,
    pub users: HashMap<String, UserData>,
    pub no_auth_enabled: bool,
    pub userpass_auth_enabled: bool,
}

impl StartupArguments {
    pub fn empty() -> Self {
        StartupArguments {
            socks5_bind_sockets: Vec::new(),
            verbose: false,
            users_file: String::new(),
            users: HashMap::new(),
            no_auth_enabled: true,
            userpass_auth_enabled: true,
        }
    }

    pub fn fill_empty_fields_with_defaults(&mut self) {
        if self.socks5_bind_sockets.is_empty() {
            self.socks5_bind_sockets
                .push(SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, DEFAULT_PORT, 0, 0)));
        }

        if self.users_file.is_empty() {
            self.users_file.push_str(DEFAULT_USERS_FILE);
        }
    }
}

#[derive(Debug)]
pub enum ArgumentsRequest {
    Help,
    Version,
    Run(StartupArguments),
}

#[derive(Debug)]
pub enum ArgumentsError {
    UknownArgument(String),
    ListenError(ListenErrorType),
    UsersFileError(UsersFileErrorType),
    NewUserError(NewUserErrorType),
    AuthToggleError(AuthToggleErrorType),
}

impl fmt::Display for ArgumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgumentsError::UknownArgument(arg) => write!(f, "Unknown argument: {arg}"),
            ArgumentsError::ListenError(listen_error) => listen_error.fmt(f),
            ArgumentsError::UsersFileError(users_file_error) => users_file_error.fmt(f),
            ArgumentsError::NewUserError(new_user_error) => new_user_error.fmt(f),
            ArgumentsError::AuthToggleError(auth_toggle_error) => auth_toggle_error.fmt(f),
        }
    }
}

#[derive(Debug)]
pub enum ListenErrorType {
    UnexpectedEnd(String),
    InvalidSocketAddress(String, String),
}

impl fmt::Display for ListenErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListenErrorType::UnexpectedEnd(arg) => write!(f, "Expected socket address after {arg}"),
            ListenErrorType::InvalidSocketAddress(arg, addr) => write!(f, "Invalid socket address after {arg}: {addr}"),
        }
    }
}

impl From<ListenErrorType> for ArgumentsError {
    fn from(value: ListenErrorType) -> Self {
        ArgumentsError::ListenError(value)
    }
}

fn parse_listen_address_arg(result: &mut StartupArguments, arg: String, maybe_arg2: Option<String>) -> Result<(), ListenErrorType> {
    let iter = match maybe_arg2 {
        Some(arg2) => match arg2.to_socket_addrs() {
            Ok(iter) => iter,
            Err(err) => {
                if err.kind() != ErrorKind::InvalidInput {
                    return Err(ListenErrorType::InvalidSocketAddress(arg, arg2));
                }

                if let Ok(iter) = format!("{arg2}:{DEFAULT_PORT}").to_socket_addrs() {
                    iter
                } else {
                    return Err(ListenErrorType::InvalidSocketAddress(arg, arg2));
                }
            }
        },
        None => return Err(ListenErrorType::UnexpectedEnd(arg)),
    };

    for sockaddr in iter {
        if !result.socks5_bind_sockets.contains(&sockaddr) {
            result.socks5_bind_sockets.push(sockaddr);
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum UsersFileErrorType {
    UnexpectedEnd(String),
    AlreadySpecified(String),
    EmptyPath(String),
}

impl fmt::Display for UsersFileErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsersFileErrorType::UnexpectedEnd(arg) => write!(f, "Expected path to users file after {arg}"),
            UsersFileErrorType::AlreadySpecified(_) => write!(f, "Only one users file may be specified"),
            UsersFileErrorType::EmptyPath(arg) => write!(f, "Empty file name after {arg}"),
        }
    }
}

impl From<UsersFileErrorType> for ArgumentsError {
    fn from(value: UsersFileErrorType) -> Self {
        ArgumentsError::UsersFileError(value)
    }
}

fn parse_users_file_arg(result: &mut StartupArguments, arg: String, maybe_arg2: Option<String>) -> Result<(), UsersFileErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(UsersFileErrorType::UnexpectedEnd(arg)),
    };

    if arg2.is_empty() {
        return Err(UsersFileErrorType::EmptyPath(arg));
    } else if !result.users_file.is_empty() {
        return Err(UsersFileErrorType::AlreadySpecified(arg));
    }

    result.users_file = arg2;
    Ok(())
}

#[derive(Debug)]
pub enum NewUserErrorType {
    UnexpectedEnd(String),
    DuplicateUsername(String, String),
    InvalidUserSpecification(String, String),
}

impl fmt::Display for NewUserErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NewUserErrorType::UnexpectedEnd(arg) => write!(f, "Expected user specification after {arg}"),
            NewUserErrorType::DuplicateUsername(arg, arg2) => write!(f, "Duplicate username at {arg} {arg2}"),
            NewUserErrorType::InvalidUserSpecification(arg, arg2) => write!(f, "Invalid user specification at {arg} {arg2}"),
        }
    }
}

impl From<NewUserErrorType> for ArgumentsError {
    fn from(value: NewUserErrorType) -> Self {
        ArgumentsError::NewUserError(value)
    }
}

#[derive(Debug)]
pub enum AuthToggleErrorType {
    UnexpectedEnd(String),
    InvalidAuthType(String, String),
}

impl fmt::Display for AuthToggleErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthToggleErrorType::UnexpectedEnd(arg) => write!(f, "Expected auth type after {arg}"),
            AuthToggleErrorType::InvalidAuthType(arg, arg2) => write!(f, "Invalid auth type at {arg} {arg2}"),
        }
    }
}

impl From<AuthToggleErrorType> for ArgumentsError {
    fn from(value: AuthToggleErrorType) -> Self {
        ArgumentsError::AuthToggleError(value)
    }
}

fn parse_new_user_arg(result: &mut StartupArguments, arg: String, maybe_arg2: Option<String>) -> Result<(), NewUserErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(NewUserErrorType::UnexpectedEnd(arg)),
    };

    let arg2_trimmed = arg2.trim();
    let starts_with_alphanumeric = arg2_trimmed.chars().next().filter(|c| c.is_alphanumeric()).is_some();
    let parse_result = if starts_with_alphanumeric {
        users::parse_line_into_user(&format!("{}{arg2_trimmed}", users::USER_PREFIX_CHAR), 1)
    } else {
        users::parse_line_into_user(arg2_trimmed, 1)
    };

    let user = match parse_result {
        Ok(Some(user)) => user,
        _ => return Err(NewUserErrorType::InvalidUserSpecification(arg, arg2)),
    };

    let vacant_entry = match result.users.entry(user.0) {
        std::collections::hash_map::Entry::Occupied(_) => return Err(NewUserErrorType::DuplicateUsername(arg, arg2)),
        std::collections::hash_map::Entry::Vacant(vac) => vac,
    };

    vacant_entry.insert(user.1);
    Ok(())
}

fn parse_auth_arg(result: &mut StartupArguments, enable: bool, arg: String, maybe_arg2: Option<String>) -> Result<(), AuthToggleErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(AuthToggleErrorType::UnexpectedEnd(arg)),
    };

    if arg2.eq_ignore_ascii_case("noauth") {
        result.no_auth_enabled = enable;
    } else if arg2.eq_ignore_ascii_case("userpass") {
        result.userpass_auth_enabled = enable;
    } else {
        return Err(AuthToggleErrorType::InvalidAuthType(arg, arg2));
    }

    Ok(())
}

pub fn parse_arguments<T>(mut args: T) -> Result<ArgumentsRequest, ArgumentsError>
where
    T: Iterator<Item = String>,
{
    let mut result = StartupArguments::empty();

    // Ignore the first argument, as it's by convention the name of the program
    args.next();

    while let Some(arg) = args.next() {
        if arg.eq_ignore_ascii_case("-h") || arg.eq_ignore_ascii_case("--help") {
            return Ok(ArgumentsRequest::Help);
        } else if arg.eq("-V") || arg.eq_ignore_ascii_case("--version") {
            return Ok(ArgumentsRequest::Version);
        } else if arg.eq("-v") || arg.eq_ignore_ascii_case("--verbose") {
            result.verbose = true;
        } else if arg.eq("-l") || arg.eq_ignore_ascii_case("--listen") {
            parse_listen_address_arg(&mut result, arg, args.next())?;
        } else if arg.eq("-U") || arg.eq_ignore_ascii_case("--users-file") {
            parse_users_file_arg(&mut result, arg, args.next())?;
        } else if arg.eq("-u") || arg.eq_ignore_ascii_case("--user") {
            parse_new_user_arg(&mut result, arg, args.next())?;
        } else if arg.eq("-a") || arg.eq_ignore_ascii_case("--auth-disable") {
            parse_auth_arg(&mut result, false, arg, args.next())?;
        } else if arg.eq("-A") || arg.eq_ignore_ascii_case("--auth-enable") {
            parse_auth_arg(&mut result, true, arg, args.next())?;
        } else {
            return Err(ArgumentsError::UknownArgument(arg));
        }
    }

    result.fill_empty_fields_with_defaults();
    Ok(ArgumentsRequest::Run(result))
}
