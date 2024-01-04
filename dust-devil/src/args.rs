//! A parser for the command-line arguments dust-devil can receive. The parser is invoked with the
//! `parse_arguments` function, which takes in an iterator of `String`s and returns a `Result` with
//! either an `ArgumentsRequest` on success, or an `ArgumentsError` on error.
//!
//! `ArgumentsRequest` is an enum with three variants; `Help`, `Version`, and
//! `Run(StartupArguments)`. This is to differentiate between when the user requests information to
//! the program, such as version or the help menu (and after displaying it the program should
//! close), or when the program should actually run a socks5 server, in which case that variant
//! provides a `StartupArguments` with the arguments parsed into a struct, including things like
//! the sockets to open, the path to the users file, which authentication methods are enabled, etc.
//! The `StartupArguments` instance is filled with default values for those not specified via
//! parameters.
//!
//! The `ArgumentsError` enum provides fine-detailed information on why the arguments are invalid.
//! This can include an unknown argument, as well as improper use of a valid argument. That said,
//! `ArgumentsError` as well as all subenums used within it implement the `fmt::Display` trait for
//! easy printing, so in order to print a human-readable explanation of why the syntax is invalid
//! a caller of `parse_arguments` may simply use `println!("{}", args_error);`.
//!
//! Additionally, the `get_version_string` and `get_help_string` functions provide human-readable
//! strings intended to be printed for their respective purposes.

use std::{
    collections::HashMap,
    fmt,
    io::ErrorKind,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
};

use dust_devil_core::users::REGULAR_PREFIX_CHAR;

use crate::users::{self, UserData};

pub const DEFAULT_USERS_FILE: &str = "users.txt";
pub const DEFAULT_SOCKS5_PORT: u16 = 1080;
pub const DEFAULT_SANDSTORM_PORT: u16 = 2222;
pub const DEFAULT_BUFFER_SIZE: u32 = 0x2000;

pub fn get_version_string() -> String {
    format!(
        concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), " ({} {})"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub fn get_help_string() -> &'static str {
    concat!(
        "Usage: dust-devil [options...]\n",
        "Options:\n",
        "  -h, --help                      Display this help menu and exit\n",
        "  -V, --version                   Display the version number and exit\n",
        "  -v, --verbose                   Display additional information while running\n",
        "  -s, --silent                    Do not print logs to stdout\n",
        "  -o, --log-file <path>           Append logs to the specified file\n",
        "  -l, --listen <address>          Specify a socket address to listen for incoming socks5 clients\n",
        "  -m, --management <address>      Specify a socket address to listen for incoming Sandstorm clients\n",
        "  -U, --users-file <path>         Load and save users to/from this file\n",
        "  -u, --user <user>               Adds a new user\n",
        "  -a, --auth-disable <auth_type>  Disables a type of authentication\n",
        "  -A, --auth-enable <auth_type>   Enables a type of authentication\n",
        "  -b, --buffer-size <size>        Sets the size of the buffer for client connections\n",
        "\n",
        "By default, the server will print logs to stdout, but not to any file. Logging may be enabled to both stdout and ",
        "to file at the same time, but keep in mind that if any of those slows down, that will slow down the server.\n",
        "\n",
        "Socket addresses may be specified as an IPv4 or IPv6 address, or a domainname, and may include a port number. ",
        "The -l/--listen and -m/--management parameter may be specified multiple times to listen on many addresses. If no ",
        "port is specified, then the default port of 1080 will be used for socks5 and 2222 for Sandstorm. If no --listen ",
        "parameter is specified, then [::]:1080 and 0.0.0.0:1080 will be used, and if no Sandstorm sockets are specified, ",
        "then [::]:2222 and 0.0.0.0:2222 will be used.\n",
        "\n",
        "Users are specified in the same format as each line on the users file, but for regular users you may drop the ",
        "role character. For example, -u \"pedro:1234\" would have the same effect as --user \"#pedro:1234\", and admins ",
        "may be added with, for example \"@admin:secret\".\n",
        "\n",
        "For enabling or disabling authentication, the available authentication types are \"noauth\" and \"userpass\".\n",
        "\n",
        "The default buffer size is 8KBs. Buffer sizes may be specified in bytes ('-b 8192'), kilobytes ('-b 8K'), ",
        "megabytes ('-b 1M') or gigabytes ('-b 1G' if you respect your computer, please don't) but may not be equal to ",
        "nor larger than 4GBs.\n",
    )
}

#[derive(Debug, PartialEq)]
pub enum ArgumentsRequest {
    Help,
    Version,
    Run(StartupArguments),
}

#[derive(Debug, PartialEq)]
pub struct StartupArguments {
    pub socks5_bind_sockets: Vec<SocketAddr>,
    pub sandstorm_bind_sockets: Vec<SocketAddr>,
    pub verbose: bool,
    pub silent: bool,
    pub log_file: Option<String>,
    pub users_file: String,
    pub users: HashMap<String, UserData>,
    pub no_auth_enabled: bool,
    pub userpass_auth_enabled: bool,
    pub buffer_size: u32,
}

impl StartupArguments {
    pub fn empty() -> Self {
        StartupArguments {
            socks5_bind_sockets: Vec::new(),
            sandstorm_bind_sockets: Vec::new(),
            verbose: false,
            silent: false,
            log_file: None,
            users_file: String::new(),
            users: HashMap::new(),
            no_auth_enabled: true,
            userpass_auth_enabled: true,
            buffer_size: 0,
        }
    }

    pub fn fill_empty_fields_with_defaults(&mut self) {
        if self.socks5_bind_sockets.is_empty() {
            self.socks5_bind_sockets
                .push(SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, DEFAULT_SOCKS5_PORT, 0, 0)));
            self.socks5_bind_sockets
                .push(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DEFAULT_SOCKS5_PORT)));
        }

        if self.sandstorm_bind_sockets.is_empty() {
            self.sandstorm_bind_sockets.push(SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::UNSPECIFIED,
                DEFAULT_SANDSTORM_PORT,
                0,
                0,
            )));
            self.sandstorm_bind_sockets
                .push(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DEFAULT_SANDSTORM_PORT)));
        }

        if self.users_file.is_empty() {
            self.users_file.push_str(DEFAULT_USERS_FILE);
        }

        if self.buffer_size == 0 {
            self.buffer_size = DEFAULT_BUFFER_SIZE;
        }
    }
}

impl Default for StartupArguments {
    fn default() -> Self {
        let mut args = Self::empty();
        args.fill_empty_fields_with_defaults();
        args
    }
}

#[derive(Debug, PartialEq)]
pub enum ArgumentsError {
    UnknownArgument(String),
    LogFileError(FileErrorType),
    Socks5ListenError(ListenErrorType),
    SandstormListenError(ListenErrorType),
    UsersFileError(FileErrorType),
    NewUserError(NewUserErrorType),
    AuthToggleError(AuthToggleErrorType),
    BufferSizeError(BufferSizeErrorType),
}

impl fmt::Display for ArgumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgumentsError::UnknownArgument(arg) => write!(f, "Unknown argument: {arg}"),
            ArgumentsError::LogFileError(log_file_error) => fmt_file_error_type(log_file_error, "log", f),
            ArgumentsError::Socks5ListenError(listen_error) => listen_error.fmt(f),
            ArgumentsError::SandstormListenError(listen_error) => listen_error.fmt(f),
            ArgumentsError::UsersFileError(users_file_error) => fmt_file_error_type(users_file_error, "users", f),
            ArgumentsError::NewUserError(new_user_error) => new_user_error.fmt(f),
            ArgumentsError::AuthToggleError(auth_toggle_error) => auth_toggle_error.fmt(f),
            ArgumentsError::BufferSizeError(buffer_size_error) => buffer_size_error.fmt(f),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum FileErrorType {
    UnexpectedEnd(String),
    AlreadySpecified(String),
    EmptyPath(String),
}

fn fmt_file_error_type(this: &FileErrorType, s: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match this {
        FileErrorType::UnexpectedEnd(arg) => write!(f, "Expected path to {s} file after {arg}"),
        FileErrorType::AlreadySpecified(_) => write!(f, "Only one {s} file may be specified"),
        FileErrorType::EmptyPath(arg) => write!(f, "Empty file name after {arg}"),
    }
}

fn parse_file_arg(result: &mut String, arg: String, maybe_arg2: Option<String>) -> Result<(), FileErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(FileErrorType::UnexpectedEnd(arg)),
    };

    if arg2.is_empty() {
        return Err(FileErrorType::EmptyPath(arg));
    } else if !result.is_empty() {
        return Err(FileErrorType::AlreadySpecified(arg));
    }

    *result = arg2;
    Ok(())
}

#[derive(Debug, PartialEq)]
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

fn parse_listen_address_arg(
    result_vec: &mut Vec<SocketAddr>,
    arg: String,
    maybe_arg2: Option<String>,
    default_port: u16,
) -> Result<(), ListenErrorType> {
    let iter = match maybe_arg2 {
        Some(arg2) => match arg2.to_socket_addrs() {
            Ok(iter) => iter,
            Err(err) => {
                if err.kind() != ErrorKind::InvalidInput {
                    return Err(ListenErrorType::InvalidSocketAddress(arg, arg2));
                }

                if let Ok(iter) = format!("{arg2}:{default_port}").to_socket_addrs() {
                    iter
                } else {
                    return Err(ListenErrorType::InvalidSocketAddress(arg, arg2));
                }
            }
        },
        None => return Err(ListenErrorType::UnexpectedEnd(arg)),
    };

    for sockaddr in iter {
        if !result_vec.contains(&sockaddr) {
            result_vec.push(sockaddr);
        }
    }

    Ok(())
}

#[derive(Debug, PartialEq)]
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

fn parse_new_user_arg(result: &mut StartupArguments, arg: String, maybe_arg2: Option<String>) -> Result<(), NewUserErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(NewUserErrorType::UnexpectedEnd(arg)),
    };

    let arg2_trimmed = arg2.trim();
    let starts_with_alphanumeric = arg2_trimmed.chars().next().filter(|c| c.is_alphanumeric()).is_some();
    let parse_result = if starts_with_alphanumeric {
        users::parse_line_into_user(&format!("{}{arg2_trimmed}", REGULAR_PREFIX_CHAR), 1, 1)
    } else {
        users::parse_line_into_user(arg2_trimmed, 1, 0)
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum BufferSizeErrorType {
    UnexpectedEnd(String),
    AlreadySpecified(String),
    InvalidSize(String, String),
}

impl fmt::Display for BufferSizeErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BufferSizeErrorType::UnexpectedEnd(arg) => write!(f, "Expected buffer size after {arg}"),
            BufferSizeErrorType::AlreadySpecified(arg) => write!(f, "Buffer size already specified at {arg}"),
            BufferSizeErrorType::InvalidSize(arg, arg2) => write!(f, "Invalid buffer size at {arg} {arg2}"),
        }
    }
}

impl From<BufferSizeErrorType> for ArgumentsError {
    fn from(value: BufferSizeErrorType) -> Self {
        ArgumentsError::BufferSizeError(value)
    }
}

fn parse_buffer_size_arg(result: &mut StartupArguments, arg: String, maybe_arg2: Option<String>) -> Result<(), BufferSizeErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(BufferSizeErrorType::UnexpectedEnd(arg)),
    };

    if result.buffer_size != 0 {
        return Err(BufferSizeErrorType::AlreadySpecified(arg));
    }

    let arg2_trimmed = arg2.trim();

    let mut iter = arg2_trimmed.chars();
    let (s, radix) = match (iter.next(), iter.next().map(|c| c.to_ascii_lowercase())) {
        (Some('0'), Some('x')) => (&arg2_trimmed[2..], 16),
        (Some('0'), Some('o')) => (&arg2_trimmed[2..], 8),
        (Some('0'), Some('b')) => (&arg2_trimmed[2..], 2),
        _ => (arg2_trimmed, 10),
    };

    let (s, multiplier) = match s.chars().last().map(|c| c.to_ascii_lowercase()) {
        Some('k') => (&s[..(s.len() - 1)], 1024),
        Some('m') => (&s[..(s.len() - 1)], 1024 * 1024),
        Some('g') => (&s[..(s.len() - 1)], 1024 * 1024 * 1024),
        _ => (s, 1),
    };

    match s.chars().next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return Err(BufferSizeErrorType::InvalidSize(arg, arg2)),
    }

    let size = match u32::from_str_radix(s, radix) {
        Ok(size) if size != 0 => size,
        _ => return Err(BufferSizeErrorType::InvalidSize(arg, arg2)),
    };

    let size = match size.checked_mul(multiplier) {
        Some(size) => size,
        None => return Err(BufferSizeErrorType::InvalidSize(arg, arg2)),
    };

    result.buffer_size = size;
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
        if arg.is_empty() {
            continue;
        } else if arg.eq_ignore_ascii_case("-h") || arg.eq_ignore_ascii_case("--help") {
            return Ok(ArgumentsRequest::Help);
        } else if arg.eq("-V") || arg.eq_ignore_ascii_case("--version") {
            return Ok(ArgumentsRequest::Version);
        } else if arg.eq("-v") || arg.eq_ignore_ascii_case("--verbose") {
            result.verbose = true;
        } else if arg.eq("-s") || arg.eq_ignore_ascii_case("--silent") {
            result.silent = true;
        } else if arg.eq("-o") || arg.eq_ignore_ascii_case("--log-file") {
            let mut log_file = result.log_file.unwrap_or_default();
            parse_file_arg(&mut log_file, arg, args.next()).map_err(ArgumentsError::LogFileError)?;
            result.log_file = Some(log_file);
        } else if arg.eq("-l") || arg.eq_ignore_ascii_case("--listen") {
            parse_listen_address_arg(&mut result.socks5_bind_sockets, arg, args.next(), DEFAULT_SOCKS5_PORT)
                .map_err(ArgumentsError::Socks5ListenError)?;
        } else if arg.eq("-m") || arg.eq_ignore_ascii_case("--management") {
            parse_listen_address_arg(&mut result.sandstorm_bind_sockets, arg, args.next(), DEFAULT_SANDSTORM_PORT)
                .map_err(ArgumentsError::SandstormListenError)?;
        } else if arg.eq("-U") || arg.eq_ignore_ascii_case("--users-file") {
            parse_file_arg(&mut result.users_file, arg, args.next()).map_err(ArgumentsError::UsersFileError)?;
        } else if arg.eq("-u") || arg.eq_ignore_ascii_case("--user") {
            parse_new_user_arg(&mut result, arg, args.next())?;
        } else if arg.eq("-a") || arg.eq_ignore_ascii_case("--auth-disable") {
            parse_auth_arg(&mut result, false, arg, args.next())?;
        } else if arg.eq("-A") || arg.eq_ignore_ascii_case("--auth-enable") {
            parse_auth_arg(&mut result, true, arg, args.next())?;
        } else if arg.eq("-b") || arg.eq_ignore_ascii_case("--buffer-size") {
            parse_buffer_size_arg(&mut result, arg, args.next())?;
        } else {
            return Err(ArgumentsError::UnknownArgument(arg));
        }
    }

    result.fill_empty_fields_with_defaults();
    Ok(ArgumentsRequest::Run(result))
}
