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
    concat!(
        "Usage: dust-devil [options...]\n",
        "Options:\n",
        "  -h, --help                      Display this help menu and exit\n",
        "  -V, --version                   Display the version number and exit\n",
        "  -v, --verbose                   Display additional information while running\n",
        "  -l, --listen <address>          Specify a socket address for listening\n",
        "  -U, --users-file <path>         Load and save users to/from this file\n",
        "  -u, --user <user>               Adds a new user\n",
        "  -a, --auth-disable <auth_type>  Disables a type of authentication\n",
        "  -A, --auth-enable <auth_type>   Enables a type of authentication\n",
        "\n",
        "Socket addresses may be specified as an IPv4 or IPv6 address, or a domainname, and may include a port number. ",
        "The --listen parameter may be specified multiple times to listen on many addresses. If no port is specified, ",
        "then the default port of 1080 will be used. If no --listen parameter is specified, then [::]:1080 will be used.\n",
        "\n",
        "Users are specified in the same format as each line on the users file, but for regular users you may drop the ",
        "role character. For example, -u \"pedro:1234\" would have the same effect as --user \"#pedro:1234\", and admins ",
        "may be added with, for example \"@admin:secret\".\n",
        "\n",
        "For enabling or disabling authentication, the available authentication types are \"noauth\" and \"userpass\".",
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
            self.socks5_bind_sockets
                .push(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DEFAULT_PORT)));
        }

        if self.users_file.is_empty() {
            self.users_file.push_str(DEFAULT_USERS_FILE);
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

#[derive(Debug, PartialEq)]
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
        if arg.is_empty() {
            continue;
        } else if arg.eq_ignore_ascii_case("-h") || arg.eq_ignore_ascii_case("--help") {
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

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    };

    use dust_devil_core::users::UserRole;

    use crate::users::UserData;

    use super::{
        parse_arguments, ArgumentsError, ArgumentsRequest, AuthToggleErrorType, ListenErrorType, NewUserErrorType, StartupArguments,
        UsersFileErrorType, DEFAULT_PORT,
    };

    fn args(s: &str) -> Result<ArgumentsRequest, ArgumentsError> {
        let iter = [String::from("./programname")]
            .into_iter()
            .chain(s.split_whitespace().map(String::from));
        parse_arguments(iter)
    }

    fn args_vec(s: &[&str]) -> Result<ArgumentsRequest, ArgumentsError> {
        let iter = [String::from("./programname")]
            .into_iter()
            .chain(s.iter().map(|&x| String::from(x)));
        parse_arguments(iter)
    }

    fn usermap(s: &[(&str, &str, UserRole)]) -> HashMap<String, UserData> {
        let mut h = HashMap::new();
        for (username, password, role) in s {
            let username = String::from(*username);
            let password = String::from(*password);
            h.insert(username, UserData { password, role: *role });
        }

        h
    }

    #[test]
    fn test_default() {
        let result = args("");
        assert_eq!(result, Ok(ArgumentsRequest::Run(StartupArguments::default())));
    }

    #[test]
    fn test_help_alone() {
        let result = args("-h");
        assert_eq!(result, Ok(ArgumentsRequest::Help));

        let result = args("--help");
        assert_eq!(result, Ok(ArgumentsRequest::Help));
    }

    #[test]
    fn test_help_last() {
        let result = args("-l localhost:1080 -u #user:pass -h");
        assert_eq!(result, Ok(ArgumentsRequest::Help));

        let result = args("-u #user:pass -v --help -l localhost:1080");
        assert_eq!(result, Ok(ArgumentsRequest::Help));
    }

    #[test]
    fn test_version_alone() {
        let result = args("-V");
        assert_eq!(result, Ok(ArgumentsRequest::Version));

        let result = args("--version");
        assert_eq!(result, Ok(ArgumentsRequest::Version));
    }

    #[test]
    fn test_version_last() {
        let result = args("-l localhost:1080 -u #user:pass -V");
        assert_eq!(result, Ok(ArgumentsRequest::Version));

        let result = args("--verbose -A noauth --version -u #petre:griffon");
        assert_eq!(result, Ok(ArgumentsRequest::Version));
    }

    #[test]
    fn test_verbose_alone() {
        let result = args("-v");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                verbose: true,
                ..Default::default()
            }))
        );

        let result = args("--verbose");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                verbose: true,
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_listen_single() {
        let result = args("-l 1.2.3.4:56789");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                socks5_bind_sockets: vec![SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 56789))],
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_listen_default_port() {
        let result = args("-l 1.2.3.4");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                socks5_bind_sockets: vec![SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), DEFAULT_PORT))],
                ..Default::default()
            }))
        );

        let result = args("-l 127.0.4.20 -l [fefe::afaf%69420]");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                socks5_bind_sockets: vec![
                    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 4, 20), DEFAULT_PORT)),
                    SocketAddr::V6(SocketAddrV6::new(
                        Ipv6Addr::new(0xfefe, 0, 0, 0, 0, 0, 0, 0xafaf),
                        DEFAULT_PORT,
                        0,
                        69420
                    )),
                ],
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_listen_multiple() {
        let result = args("-l [abcd::4f5:2e2e:4321:3ac3%69]:7164 -l 1.2.3.4:56789");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                socks5_bind_sockets: vec![
                    SocketAddr::V6(SocketAddrV6::new(
                        Ipv6Addr::new(0xabcd, 0, 0, 0, 0x04f5, 0x2e2e, 0x4321, 0x3ac3),
                        7164,
                        0,
                        69
                    )),
                    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 56789)),
                ],
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_listen_unexpected_end() {
        let result = args("-l");
        assert_eq!(
            result,
            Err(ArgumentsError::ListenError(ListenErrorType::UnexpectedEnd("-l".to_string())))
        );

        let result = args("--listen");
        assert_eq!(
            result,
            Err(ArgumentsError::ListenError(ListenErrorType::UnexpectedEnd("--listen".to_string())))
        );
    }

    #[test]
    fn test_listen_bad_format() {
        let result = args("-l 127.420.666.0");
        assert_eq!(
            result,
            Err(ArgumentsError::ListenError(ListenErrorType::InvalidSocketAddress(
                "-l".to_string(),
                "127.420.666.0".to_string()
            )))
        );

        let result = args("--listen [fafa::fefe:fifi:fofo:fufu]");
        assert_eq!(
            result,
            Err(ArgumentsError::ListenError(ListenErrorType::InvalidSocketAddress(
                "--listen".to_string(),
                "[fafa::fefe:fifi:fofo:fufu]".to_string()
            )))
        );

        let result = args_vec(&["--listen", "alto chori ameo 🤩🤩"]);
        assert_eq!(
            result,
            Err(ArgumentsError::ListenError(ListenErrorType::InvalidSocketAddress(
                "--listen".to_string(),
                "alto chori ameo 🤩🤩".to_string()
            )))
        );
    }

    #[test]
    fn test_users_file() {
        let result = args("-U ./some/dir/file.txt");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users_file: "./some/dir/file.txt".to_string(),
                ..Default::default()
            }))
        );

        let result = args("--users-file ./some/dir/file.txt");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users_file: "./some/dir/file.txt".to_string(),
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_users_file_empty() {
        let result = args_vec(&["-U", ""]);
        assert_eq!(
            result,
            Err(ArgumentsError::UsersFileError(UsersFileErrorType::EmptyPath("-U".to_string())))
        );

        let result = args_vec(&["--users-file", ""]);
        assert_eq!(
            result,
            Err(ArgumentsError::UsersFileError(UsersFileErrorType::EmptyPath(
                "--users-file".to_string()
            )))
        );
    }

    #[test]
    fn test_users_file_unexpected_end() {
        let result = args("-U");
        assert_eq!(
            result,
            Err(ArgumentsError::UsersFileError(UsersFileErrorType::UnexpectedEnd("-U".to_string())))
        );

        let result = args("--users-file");
        assert_eq!(
            result,
            Err(ArgumentsError::UsersFileError(UsersFileErrorType::UnexpectedEnd(
                "--users-file".to_string()
            )))
        );
    }

    #[test]
    fn test_users_file_specified_twice() {
        let result = args("-U ./my_users -v --users-file againnnn");
        assert_eq!(
            result,
            Err(ArgumentsError::UsersFileError(UsersFileErrorType::AlreadySpecified(
                "--users-file".to_string()
            )))
        );

        let result = args("--users-file ./my_users -v -U againnnn");
        assert_eq!(
            result,
            Err(ArgumentsError::UsersFileError(UsersFileErrorType::AlreadySpecified(
                "-U".to_string()
            )))
        );
    }

    #[test]
    fn test_user_single_regular() {
        let result = args("-u petre:griffon");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users: usermap(&[("petre", "griffon", UserRole::Regular)]),
                ..Default::default()
            }))
        );

        let result = args("-u #petre:griffon");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users: usermap(&[("petre", "griffon", UserRole::Regular)]),
                ..Default::default()
            }))
        );

        let result = args("--user per$te:groff:ofo");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users: usermap(&[("per$te", "groff:ofo", UserRole::Regular)]),
                ..Default::default()
            }))
        );

        let result = args("--user #perte:groffofo");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users: usermap(&[("perte", "groffofo", UserRole::Regular)]),
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_user_single_admin() {
        let result = args("--user @pe#rper:gor=fon");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users: usermap(&[("pe#rper", "gor=fon", UserRole::Admin)]),
                ..Default::default()
            }))
        );

        let result = args("--user @PeréPe:GoroFoFo");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users: usermap(&[("PeréPe", "GoroFoFo", UserRole::Admin)]),
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_user_multiple_complex_names() {
        let result = args("-u ##pé\\:çá\\:'h\\\\**\\:\\::@=:::\\\\NíÇ --user @👋h\\:e\\:llo\\\\_wÚrl?d:@@👍👍ÁÇçE💀fórgôr💀💀");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users: usermap(&[
                    ("#pé:çá:'h\\**::", "@=:::\\NíÇ", UserRole::Regular),
                    ("👋h:e:llo\\_wÚrl?d", "@@👍👍ÁÇçE💀fórgôr💀💀", UserRole::Admin),
                ]),
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_user_unexpected_end() {
        let result = args("-u");
        assert_eq!(
            result,
            Err(ArgumentsError::NewUserError(NewUserErrorType::UnexpectedEnd("-u".to_string())))
        );
    }

    #[test]
    fn test_user_duplicate_username() {
        let result = args("-u #pedro:pedro -u pedró:pedro --user @pedro:pedro");
        assert_eq!(
            result,
            Err(ArgumentsError::NewUserError(NewUserErrorType::DuplicateUsername(
                "--user".to_string(),
                "@pedro:pedro".to_string()
            )))
        );
    }

    #[test]
    fn test_user_field_too_long() {
        let arg = "-u #".to_string() + &"a".repeat(255) + ":" + &"b".repeat(255);
        let result = args(&arg);
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users: usermap(&[(&"a".repeat(255), &"b".repeat(255), UserRole::Regular)]),
                ..Default::default()
            }))
        );

        let arg = "--user #".to_string() + &"a".repeat(256) + ":" + &"b".repeat(255);
        let result = args(&arg);
        assert_eq!(
            result,
            Err(ArgumentsError::NewUserError(NewUserErrorType::InvalidUserSpecification(
                "--user".to_string(),
                arg[7..].to_string()
            )))
        );

        let arg = "-u #".to_string() + &"a".repeat(255) + ":" + &"b".repeat(256);
        let result = args(&arg);
        assert_eq!(
            result,
            Err(ArgumentsError::NewUserError(NewUserErrorType::InvalidUserSpecification(
                "-u".to_string(),
                arg[3..].to_string()
            )))
        );
    }

    #[test]
    fn test_auth_disable() {
        let result = args("-a noauth");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                no_auth_enabled: false,
                ..Default::default()
            }))
        );

        let result = args("--auth-disable noauth -a userpass");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                no_auth_enabled: false,
                userpass_auth_enabled: false,
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_auth_enable() {
        let result = args("-A noauth");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                no_auth_enabled: true,
                ..Default::default()
            }))
        );

        let result = args("--auth-enable noauth -A userpass");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                no_auth_enabled: true,
                userpass_auth_enabled: true,
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_auth_unexpected_end() {
        let result = args("-a");
        assert_eq!(
            result,
            Err(ArgumentsError::AuthToggleError(AuthToggleErrorType::UnexpectedEnd(
                "-a".to_string()
            )))
        );

        let result = args("--auth-disable");
        assert_eq!(
            result,
            Err(ArgumentsError::AuthToggleError(AuthToggleErrorType::UnexpectedEnd(
                "--auth-disable".to_string()
            )))
        );

        let result = args("-A");
        assert_eq!(
            result,
            Err(ArgumentsError::AuthToggleError(AuthToggleErrorType::UnexpectedEnd(
                "-A".to_string()
            )))
        );

        let result = args("--auth-enable");
        assert_eq!(
            result,
            Err(ArgumentsError::AuthToggleError(AuthToggleErrorType::UnexpectedEnd(
                "--auth-enable".to_string()
            )))
        );
    }

    #[test]
    fn test_auth_invalid_types() {
        let result = args("-a noauthh");
        assert_eq!(
            result,
            Err(ArgumentsError::AuthToggleError(AuthToggleErrorType::InvalidAuthType(
                "-a".to_string(),
                "noauthh".to_string()
            )))
        );

        let result = args("-A usempass");
        assert_eq!(
            result,
            Err(ArgumentsError::AuthToggleError(AuthToggleErrorType::InvalidAuthType(
                "-A".to_string(),
                "usempass".to_string()
            )))
        );

        let result = args("--auth-disable marcos");
        assert_eq!(
            result,
            Err(ArgumentsError::AuthToggleError(AuthToggleErrorType::InvalidAuthType(
                "--auth-disable".to_string(),
                "marcos".to_string()
            )))
        );

        let result = args("--auth-enable cucurucho");
        assert_eq!(
            result,
            Err(ArgumentsError::AuthToggleError(AuthToggleErrorType::InvalidAuthType(
                "--auth-enable".to_string(),
                "cucurucho".to_string()
            )))
        );
    }

    #[test]
    fn test_integration1() {
        let result = args("-l 0.0.0.0 -v -u #pedro:pedro -l [::1%6969]:6060 -U myfile.txt --auth-disable noauth -u @\\\\so\\:co:tr\\\\oco");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                socks5_bind_sockets: vec![
                    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DEFAULT_PORT)),
                    SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 6060, 0, 6969)),
                ],
                verbose: true,
                users: usermap(&[("pedro", "pedro", UserRole::Regular), ("\\so:co", "tr\\oco", UserRole::Admin),]),
                users_file: "myfile.txt".to_string(),
                no_auth_enabled: false,
                ..Default::default()
            }))
        );
    }

    #[test]
    fn test_integration2() {
        let result = args("-U picante.txt --auth-enable noauth -u juan:carlos -v -u #carlos:juan --auth-disable userpass -l 1.2.3.4:5678");
        assert_eq!(
            result,
            Ok(ArgumentsRequest::Run(StartupArguments {
                users_file: "picante.txt".to_string(),
                no_auth_enabled: true,
                userpass_auth_enabled: false,
                users: usermap(&[("juan", "carlos", UserRole::Regular), ("carlos", "juan", UserRole::Regular),]),
                verbose: true,
                socks5_bind_sockets: vec![SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 5678))],
            }))
        );
    }
}
