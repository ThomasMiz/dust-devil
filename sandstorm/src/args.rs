use std::{
    env, fmt,
    io::ErrorKind,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
};

use dust_devil_core::{
    socks5::AuthMethod,
    users::{self, UserRole},
};

pub const DEFAULT_SOCKS5_PORT: u16 = 1080;
pub const DEFAULT_SANDSTORM_PORT: u16 = 2222;
pub const CREDENTIALS_ENV_VARIABLE: &str = "SANDSTORM_USER";

pub fn get_version_string() -> String {
    format!(
        concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), " ({} {})"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub fn get_help_string() -> &'static str {
    concat!(
        "Usage: sandstorm [options...]\n",
        "Options:\n",
        "  -h, --help                      Display this help menu and exit\n",
        "  -V, --version                   Display the version number and exit\n",
        "  -v, --verbose                   Display additional information while running\n",
        "  -s, --silent                    Do not print to stdout\n",
        "  -x, --host <address>            Specify the server to connect to\n",
        "  -c, --credentials <creds>       Specify the user to log in as, in user:password format\n",
        "  -S, --shutdown                  Requests the server to shut down\n",
        "  -l, --list-socks5               Requests the server sends a list of socks5 sockets\n",
        "  -k, --add-socks5 <address>      Requests the server opens a new socks5 socket\n",
        "  -r, --remove-socks5 <address>   Requests the server removes an existing socks5 socket\n",
        "  -L, --list-sandstr              Requests the server sends a list of Sandstorm sockets\n",
        "  -K, --add-sandstr <address>     Requests the server opens a new Sandstorm socket\n",
        "  -R, --remove-sandstr <address>  Requests the server removes an existing Sandstorm socket\n",
        "  -t, --list-users                Requests the server adds a new user\n",
        "  -u, --add-user <user>           Requests the server adds a new user\n",
        "  -p, --update-user <updt_user>   Requests the server updates an existing user\n",
        "  -d, --delete-user <username>    Requests the server deletes an existing user\n",
        "  -z, --list-auth                 Requests the server sends a list of auth methods\n",
        "  -A, --auth-enable <auth_type>   Requests the server enables an authentication method\n",
        "  -a, --auth-disable <auth_type>  Requests the server disables an authentication method\n",
        "  -m, --get-metrics               Requests the server sends the current metrics\n",
        "  -B, --get-buffer-size           Requests the server sends the current buffer size\n",
        "  -b, --set-buffer-size <size>    Requests the server changes its buffer size\n",
        "  -w, --meow                      Requests a meow ping to the server\n",
        "  -o, --output-logs               Remain open and print the server's logs to stdout\n",
        "  -i, --interactive               Remains open with an advanced terminal UI interface\n",
        "\n",
        "Socket addresses may be specified as an IPv4 or IPv6 address, or a domainname, and may include a port number. If ",
        "no port is specified, then the appropriate default will be used (1080 for Socks5 and 2222 for Sandstorm). If no ",
        "-x/--host parameter is specified, then localhost:2222 will be used.\n",
        "\n",
        "Credentials may be specified with the -c/--credentials argument, in username:password format. If no credentials ",
        "argument is specified, then the credentials will be taken from the SANDSTORM_USER environment variable, which must ",
        "follow the same format.\n",
        "\n",
        "When adding a user, it is specified in the (role)?user:password format. For example, \"#carlos:1234\" represents a ",
        "regular user with username \"carlos\" and password \"1234\", and \"@josé:4:4:4\" represents an admin user with ",
        "username \"josé\" and password \"4:4:4\". If the role char is omitted, then a regular user is assumed. Updating an ",
        "existing user work much the same way, but the role char or password may be omitted. Only the fields present will be ",
        "updated, those omitted will not be modified. To specify an username that contains a ':' character, you may escape ",
        "it like so: \"#chi\\:chí:4:3:2:1\" (this produces a regular user \"chi:chí\" with password \"4:3:2:1\"). When ",
        "deleting an user, no escaping is necessary, as only the username is specified.",
        "\n",
        "For enabling or disabling authentication, the available authentication types are \"noauth\" and \"userpass\".\n",
        "\n",
        "Buffer sizes may be specified in bytes ('-b 8192'), kilobytes ('-b 8K'), megabytes ('-b 1M') or gigabytes ('-b 1G' ",
        "if you respect your computer, please don't) but may not be equal to nor larger than 4GBs.\n",
        "\n",
        "The requests are done in the order in which they're specified and their results printed to stdout (unless ",
        "-s/--silent is specified). Pipelining will be used, so the requests are not guaranteed to come back in the same ",
        "order. The only ordering guarantees are those defined in the Sandstorm protocol (so, for example, list/add/remove ",
        "socks5 sockets operations are guaranteed to be handled in order and answered in order, but an add user request in ",
        "the middle of all that may not come back in the same order.\n",
        "\n",
        "The -o/--output-logs and -i/--interactive modes are mutually exclusive, only one may be enabled.\n"
    )
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentsRequest {
    Help,
    Version,
    Run(StartupArguments),
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommandRequest {
    Shutdown,
    ListSocks5Sockets,
    AddSocks5Socket(SocketAddr),
    RemoveSocks5Socket(SocketAddr),
    ListSandstormSockets,
    AddSandstormSocket(SocketAddr),
    RemoveSandstormSocket(SocketAddr),
    ListUsers,
    AddUser(String, String, UserRole),
    UpdateUser(String, Option<String>, Option<UserRole>),
    DeleteUser(String),
    ListAuthMethods,
    ToggleAuthMethod(AuthMethod, bool),
    GetMetrics,
    GetBufferSize,
    SetBufferSize(u32),
    Meow,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StartupArguments {
    pub verbose: bool,
    pub silent: bool,
    pub server_address: Vec<SocketAddr>,
    pub login_credentials: (String, String),
    pub requests: Vec<CommandRequest>,
    pub output_logs: bool,
    pub interactive: bool,
}

impl StartupArguments {
    pub fn empty() -> Self {
        StartupArguments {
            verbose: false,
            silent: false,
            server_address: Vec::new(),
            login_credentials: (String::new(), String::new()),
            requests: Vec::new(),
            output_logs: false,
            interactive: false,
        }
    }

    pub fn fill_empty_fields_with_defaults(&mut self) {
        if self.server_address.is_empty() {
            self.server_address
                .push(SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, DEFAULT_SANDSTORM_PORT, 0, 0)));
            self.server_address
                .push(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEFAULT_SANDSTORM_PORT)));
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

#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentsError {
    UnknownArgument(String),
    HostError(SocketErrorType),
    CredentialsError(CredentialsErrorType),
    EnvCredentialsError(EnvCredentialsErrorType),
    NoCredentialsSpecified,
    AddSocks5Error(SocketErrorType),
    RemoveSocks5Error(SocketErrorType),
    AddSandstormError(SocketErrorType),
    RemoveSandstormError(SocketErrorType),
    AddUserError(AddUserErrorType),
    UpdateUserError(UpdateUserErrorType),
    DeleteUserError(DeleteUserErrorType),
    AuthToggleError(AuthToggleErrorType),
    BufferSizeError(BufferSizeErrorType),
    CantMixOutputAndInteractive,
}

impl fmt::Display for ArgumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArgument(arg) => write!(f, "Unknown argument: {arg}"),
            Self::HostError(host_error) => host_error.fmt(f),
            Self::CredentialsError(credentials_error) => credentials_error.fmt(f),
            Self::EnvCredentialsError(env_credentials_error) => env_credentials_error.fmt(f),
            Self::NoCredentialsSpecified => write!(f, "No credentials specified. Use -c/--credentials username:password, or set the {CREDENTIALS_ENV_VARIABLE} environment variable"),
            Self::AddSocks5Error(add_socks5_error) => add_socks5_error.fmt(f),
            Self::RemoveSocks5Error(remove_socks5_error) => remove_socks5_error.fmt(f),
            Self::AddSandstormError(add_sandstorm_error) => add_sandstorm_error.fmt(f),
            Self::RemoveSandstormError(remove_sandstorm_error) => remove_sandstorm_error.fmt(f),
            Self::AddUserError(add_user_error) => add_user_error.fmt(f),
            Self::UpdateUserError(update_user_error) => update_user_error.fmt(f),
            Self::DeleteUserError(delete_user_error) => delete_user_error.fmt(f),
            Self::AuthToggleError(auth_toggle_error) => auth_toggle_error.fmt(f),
            Self::BufferSizeError(buffer_size_error) => buffer_size_error.fmt(f),
            Self::CantMixOutputAndInteractive => write!(f, "Cannot specify both -o/--output-logs and -i/--interactive together"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SocketErrorType {
    UnexpectedEnd(String),
    InvalidSocketAddress(String, String),
}

impl fmt::Display for SocketErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected socket address after {arg}"),
            Self::InvalidSocketAddress(arg, addr) => write!(f, "Invalid socket address after {arg}: {addr}"),
        }
    }
}

fn parse_socket_arg(
    result_vec: &mut Vec<SocketAddr>,
    arg: String,
    maybe_arg2: Option<String>,
    default_port: u16,
) -> Result<(), SocketErrorType> {
    let arg2 = match maybe_arg2 {
        Some(value) => value,
        None => return Err(SocketErrorType::UnexpectedEnd(arg)),
    };

    let iter = match arg2.to_socket_addrs() {
        Ok(iter) => iter,
        Err(err) if err.kind() == ErrorKind::InvalidInput => match format!("{arg2}:{default_port}").to_socket_addrs() {
            Ok(iter) => iter,
            Err(_) => return Err(SocketErrorType::InvalidSocketAddress(arg, arg2)),
        },
        Err(_) => return Err(SocketErrorType::InvalidSocketAddress(arg, arg2)),
    };

    for sockaddr in iter {
        if !result_vec.contains(&sockaddr) {
            result_vec.push(sockaddr);
        }
    }

    Ok(())
}

pub enum ParseIntoUserError {
    Empty,
    InvalidRoleChar(char),
    EmptyUsername,
    UsernameTooLong,
    EmptyPassword,
    PasswordTooLong,
}

fn parse_into_user(s: &str) -> Result<(String, Option<String>, Option<UserRole>), ParseIntoUserError> {
    let mut chars = s.chars();

    let first_char = chars.next().ok_or(ParseIntoUserError::Empty)?;

    let mut username = String::with_capacity(255);
    let mut escape_next = false;

    let maybe_role = if first_char.is_alphanumeric() {
        username.push(first_char);
        None
    } else {
        match first_char {
            users::ADMIN_PREFIX_CHAR => Some(UserRole::Admin),
            users::REGULAR_PREFIX_CHAR => Some(UserRole::Regular),
            users::ESCAPE_CHAR => {
                escape_next = true;
                None
            }
            _ => return Err(ParseIntoUserError::InvalidRoleChar(first_char)),
        }
    };

    loop {
        let next_char = match chars.next() {
            Some(c) => c,
            None => {
                if username.is_empty() {
                    return Err(ParseIntoUserError::EmptyUsername);
                } else {
                    return Ok((username, None, maybe_role));
                }
            }
        };

        if escape_next || (next_char != users::ESCAPE_CHAR && next_char != ':') {
            if username.len() >= 255 {
                return Err(ParseIntoUserError::UsernameTooLong);
            }
            username.push(next_char);
        }

        if escape_next {
            escape_next = false;
        } else if next_char == users::ESCAPE_CHAR {
            escape_next = true;
        } else if next_char == ':' {
            break;
        }
    }

    if username.is_empty() {
        return Err(ParseIntoUserError::EmptyUsername);
    }

    let mut password = String::with_capacity(255);
    let mut escape_next = false;
    for next_char in chars {
        if escape_next || next_char != users::ESCAPE_CHAR {
            if password.len() >= 255 {
                return Err(ParseIntoUserError::PasswordTooLong);
            }
            password.push(next_char);
        }

        if escape_next {
            escape_next = false;
        } else if next_char == users::ESCAPE_CHAR {
            escape_next = true;
        }
    }

    if password.is_empty() {
        return Err(ParseIntoUserError::EmptyPassword);
    }

    Ok((username, Some(password), maybe_role))
}

#[derive(Debug, PartialEq, Eq)]
pub enum CredentialsErrorType {
    UnexpectedEnd(String),
    AlreadySpecified(String),
    Empty(String),
    DontSpecifyRoleChar(String, char),
    EmptyUsername(String),
    UsernameTooLong(String),
    EmptyPassword(String),
    PasswordTooLong(String),
}

impl fmt::Display for CredentialsErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected credentials after {arg}"),
            Self::AlreadySpecified(arg) => write!(f, "Credentials already specified at {arg}"),
            Self::Empty(arg) => write!(f, "Empty credentials argument after {arg}"),
            Self::DontSpecifyRoleChar(arg, role_char) => {
                write!(f, "Credentials after {arg} shouldn't have a role char, remove the '{role_char}'")
            }
            Self::EmptyUsername(arg) => write!(f, "Credentials after {arg} have no username"),
            Self::UsernameTooLong(arg) => write!(f, "Credentials after {arg} username too long"),
            Self::EmptyPassword(arg) => write!(f, "Credentials after {arg} have no password"),
            Self::PasswordTooLong(arg) => write!(f, "Credentials after {arg} password too long"),
        }
    }
}

impl From<CredentialsErrorType> for ArgumentsError {
    fn from(value: CredentialsErrorType) -> Self {
        Self::CredentialsError(value)
    }
}

fn parse_credentials(result: &mut (String, String), arg: String, maybe_arg2: Option<String>) -> Result<(), CredentialsErrorType> {
    if !result.0.is_empty() {
        return Err(CredentialsErrorType::AlreadySpecified(arg));
    }

    let credentials = match maybe_arg2 {
        Some(creds) => creds,
        None => return Err(CredentialsErrorType::UnexpectedEnd(arg)),
    };

    let creds_tuple = match parse_into_user(&credentials) {
        Ok((username, Some(password), None)) => Ok((username, password)),
        Ok((_, _, Some(role))) => Err(CredentialsErrorType::DontSpecifyRoleChar(arg, role.into_role_char())),
        Ok((_, None, _)) => Err(CredentialsErrorType::EmptyPassword(arg)),
        Err(ParseIntoUserError::Empty) => Err(CredentialsErrorType::Empty(arg)),
        Err(ParseIntoUserError::InvalidRoleChar(role_char)) => Err(CredentialsErrorType::DontSpecifyRoleChar(arg, role_char)),
        Err(ParseIntoUserError::EmptyUsername) => Err(CredentialsErrorType::EmptyUsername(arg)),
        Err(ParseIntoUserError::UsernameTooLong) => Err(CredentialsErrorType::UsernameTooLong(arg)),
        Err(ParseIntoUserError::EmptyPassword) => Err(CredentialsErrorType::EmptyPassword(arg)),
        Err(ParseIntoUserError::PasswordTooLong) => Err(CredentialsErrorType::PasswordTooLong(arg)),
    }?;

    *result = creds_tuple;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum EnvCredentialsErrorType {
    Empty,
    NotUnicode,
    DontSpecifyRoleChar(char),
    EmptyUsername,
    UsernameTooLong,
    EmptyPassword,
    PasswordTooLong,
}

impl fmt::Display for EnvCredentialsErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Empty credentials at env variable {CREDENTIALS_ENV_VARIABLE}"),
            Self::NotUnicode => write!(f, "Env variable {CREDENTIALS_ENV_VARIABLE} exists, but is not valid Unicode"),
            Self::DontSpecifyRoleChar(role_char) => {
                write!(
                    f,
                    "Credentials at env variable {CREDENTIALS_ENV_VARIABLE} shouldn't have a role char, remove the '{role_char}'"
                )
            }
            Self::EmptyUsername => write!(f, "Credentials at env variable {CREDENTIALS_ENV_VARIABLE} have no username"),
            Self::UsernameTooLong => write!(f, "Credentials at env variable {CREDENTIALS_ENV_VARIABLE} username too long"),
            Self::EmptyPassword => write!(f, "Credentials at env variable {CREDENTIALS_ENV_VARIABLE} have no password"),
            Self::PasswordTooLong => write!(f, "Credentials at env variable {CREDENTIALS_ENV_VARIABLE} password too long"),
        }
    }
}

impl From<EnvCredentialsErrorType> for ArgumentsError {
    fn from(value: EnvCredentialsErrorType) -> Self {
        Self::EnvCredentialsError(value)
    }
}

fn parse_env_credentials() -> Result<Option<(String, String)>, EnvCredentialsErrorType> {
    let credentials = match env::var(CREDENTIALS_ENV_VARIABLE) {
        Ok(s) => s,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(env::VarError::NotUnicode(_)) => return Err(EnvCredentialsErrorType::NotUnicode),
    };

    match parse_into_user(&credentials) {
        Ok((username, Some(password), None)) => Ok(Some((username, password))),
        Ok((_, _, Some(role))) => Err(EnvCredentialsErrorType::DontSpecifyRoleChar(role.into_role_char())),
        Ok((_, None, _)) => Err(EnvCredentialsErrorType::EmptyPassword),
        Err(ParseIntoUserError::Empty) => Err(EnvCredentialsErrorType::Empty),
        Err(ParseIntoUserError::InvalidRoleChar(role_char)) => Err(EnvCredentialsErrorType::DontSpecifyRoleChar(role_char)),
        Err(ParseIntoUserError::EmptyUsername) => Err(EnvCredentialsErrorType::EmptyUsername),
        Err(ParseIntoUserError::UsernameTooLong) => Err(EnvCredentialsErrorType::UsernameTooLong),
        Err(ParseIntoUserError::EmptyPassword) => Err(EnvCredentialsErrorType::EmptyPassword),
        Err(ParseIntoUserError::PasswordTooLong) => Err(EnvCredentialsErrorType::PasswordTooLong),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AddUserErrorType {
    UnexpectedEnd(String),
    Empty(String),
    MissingPassword(String),
    InvalidRoleChar(String, char),
    EmptyUsername(String),
    UsernameTooLong(String),
    EmptyPassword(String),
    PasswordTooLong(String),
}

impl fmt::Display for AddUserErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected user after {arg}"),
            Self::Empty(arg) => write!(f, "Empty add user argument after {arg}"),
            Self::MissingPassword(arg) => write!(f, "User after {arg} must have a password"),
            Self::InvalidRoleChar(arg, role_char) => write!(f, "User after {arg} has invalid role char '{role_char}'"),
            Self::EmptyUsername(arg) => write!(f, "User after {arg} has no username"),
            Self::UsernameTooLong(arg) => write!(f, "User after {arg} username too long"),
            Self::EmptyPassword(arg) => write!(f, "User after {arg} has no password"),
            Self::PasswordTooLong(arg) => write!(f, "User after {arg} password too long"),
        }
    }
}

impl From<AddUserErrorType> for ArgumentsError {
    fn from(value: AddUserErrorType) -> Self {
        Self::AddUserError(value)
    }
}

fn parse_add_user(arg: String, maybe_arg2: Option<String>) -> Result<(String, String, UserRole), AddUserErrorType> {
    let user_string = match maybe_arg2 {
        Some(s) => s,
        None => return Err(AddUserErrorType::UnexpectedEnd(arg)),
    };

    match parse_into_user(&user_string) {
        Ok((username, Some(password), maybe_role)) => Ok((username, password, maybe_role.unwrap_or(UserRole::Regular))),
        Ok((_, None, _)) => Err(AddUserErrorType::MissingPassword(arg)),
        Err(ParseIntoUserError::Empty) => Err(AddUserErrorType::Empty(arg)),
        Err(ParseIntoUserError::InvalidRoleChar(role_char)) => Err(AddUserErrorType::InvalidRoleChar(arg, role_char)),
        Err(ParseIntoUserError::EmptyUsername) => Err(AddUserErrorType::EmptyUsername(arg)),
        Err(ParseIntoUserError::UsernameTooLong) => Err(AddUserErrorType::UsernameTooLong(arg)),
        Err(ParseIntoUserError::EmptyPassword) => Err(AddUserErrorType::EmptyPassword(arg)),
        Err(ParseIntoUserError::PasswordTooLong) => Err(AddUserErrorType::PasswordTooLong(arg)),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateUserErrorType {
    UnexpectedEnd(String),
    Empty(String),
    NothingWasSpecified(String),
    InvalidRoleChar(String, char),
    EmptyUsername(String),
    UsernameTooLong(String),
    EmptyPassword(String),
    PasswordTooLong(String),
}

impl fmt::Display for UpdateUserErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected user after {arg}"),
            Self::Empty(arg) => write!(f, "Empty update user argument after {arg}"),
            Self::NothingWasSpecified(arg) => write!(f, "User after {arg} must have a role or a password"),
            Self::InvalidRoleChar(arg, role_char) => write!(f, "User after {arg} has invalid role char '{role_char}'"),
            Self::EmptyUsername(arg) => write!(f, "User after {arg} has no username"),
            Self::UsernameTooLong(arg) => write!(f, "User after {arg} username too long"),
            Self::EmptyPassword(arg) => write!(f, "User after {arg} has no password"),
            Self::PasswordTooLong(arg) => write!(f, "User after {arg} password too long"),
        }
    }
}

impl From<UpdateUserErrorType> for ArgumentsError {
    fn from(value: UpdateUserErrorType) -> Self {
        Self::UpdateUserError(value)
    }
}

fn parse_update_user(arg: String, maybe_arg2: Option<String>) -> Result<(String, Option<String>, Option<UserRole>), UpdateUserErrorType> {
    let user_string = match maybe_arg2 {
        Some(s) => s,
        None => return Err(UpdateUserErrorType::UnexpectedEnd(arg)),
    };

    match parse_into_user(&user_string) {
        Ok((_, None, None)) => Err(UpdateUserErrorType::NothingWasSpecified(arg)),
        Ok((username, maybe_password, maybe_role)) => Ok((username, maybe_password, maybe_role)),
        Err(ParseIntoUserError::Empty) => Err(UpdateUserErrorType::Empty(arg)),
        Err(ParseIntoUserError::InvalidRoleChar(role_char)) => Err(UpdateUserErrorType::InvalidRoleChar(arg, role_char)),
        Err(ParseIntoUserError::EmptyUsername) => Err(UpdateUserErrorType::EmptyUsername(arg)),
        Err(ParseIntoUserError::UsernameTooLong) => Err(UpdateUserErrorType::UsernameTooLong(arg)),
        Err(ParseIntoUserError::EmptyPassword) => Err(UpdateUserErrorType::EmptyPassword(arg)),
        Err(ParseIntoUserError::PasswordTooLong) => Err(UpdateUserErrorType::PasswordTooLong(arg)),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DeleteUserErrorType {
    UnexpectedEnd(String),
    Empty(String),
    UsernameTooLong(String),
}

impl fmt::Display for DeleteUserErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected username after {arg}"),
            Self::Empty(arg) => write!(f, "Empty delete user argument after {arg}"),
            Self::UsernameTooLong(arg) => write!(f, "Username after {arg} too long"),
        }
    }
}

impl From<DeleteUserErrorType> for ArgumentsError {
    fn from(value: DeleteUserErrorType) -> Self {
        Self::DeleteUserError(value)
    }
}

fn parse_delete_user(arg: String, maybe_arg2: Option<String>) -> Result<String, DeleteUserErrorType> {
    match maybe_arg2 {
        None => Err(DeleteUserErrorType::UnexpectedEnd(arg)),
        Some(username) if username.is_empty() => Err(DeleteUserErrorType::Empty(arg)),
        Some(username) if username.len() > 255 => Err(DeleteUserErrorType::UsernameTooLong(arg)),
        Some(username) => Ok(username),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AuthToggleErrorType {
    UnexpectedEnd(String),
    InvalidAuthType(String, String),
}

impl fmt::Display for AuthToggleErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected auth type after {arg}"),
            Self::InvalidAuthType(arg, arg2) => write!(f, "Invalid auth type at {arg} {arg2}"),
        }
    }
}

impl From<AuthToggleErrorType> for ArgumentsError {
    fn from(value: AuthToggleErrorType) -> Self {
        Self::AuthToggleError(value)
    }
}

fn parse_auth_arg(arg: String, maybe_arg2: Option<String>) -> Result<AuthMethod, AuthToggleErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(AuthToggleErrorType::UnexpectedEnd(arg)),
    };

    if arg2.eq_ignore_ascii_case("noauth") {
        Ok(AuthMethod::NoAuth)
    } else if arg2.eq_ignore_ascii_case("userpass") {
        Ok(AuthMethod::UsernameAndPassword)
    } else {
        Err(AuthToggleErrorType::InvalidAuthType(arg, arg2))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BufferSizeErrorType {
    UnexpectedEnd(String),
    Empty(String),
    InvalidSize(String, String),
}

impl fmt::Display for BufferSizeErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected buffer size after {arg}"),
            Self::Empty(arg) => write!(f, "Empty buffer size argument after {arg}"),
            Self::InvalidSize(arg, arg2) => write!(f, "Invalid buffer size at {arg} {arg2}"),
        }
    }
}

impl From<BufferSizeErrorType> for ArgumentsError {
    fn from(value: BufferSizeErrorType) -> Self {
        Self::BufferSizeError(value)
    }
}

fn parse_buffer_size_arg(arg: String, maybe_arg2: Option<String>) -> Result<u32, BufferSizeErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(BufferSizeErrorType::UnexpectedEnd(arg)),
    };

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
        None => return Err(BufferSizeErrorType::Empty(arg)),
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

    Ok(size)
}

pub fn parse_arguments<T>(mut args: T) -> Result<ArgumentsRequest, ArgumentsError>
where
    T: Iterator<Item = String>,
{
    let mut result = StartupArguments::empty();

    // Ignore the first argument, as it's by convention the name of the program
    args.next();
    let mut tmp_sockets_vec = Vec::new();

    while let Some(arg) = args.next() {
        if arg.is_empty() {
            continue;
        } else if arg.eq("-h") || arg.eq_ignore_ascii_case("--help") {
            return Ok(ArgumentsRequest::Help);
        } else if arg.eq("-V") || arg.eq_ignore_ascii_case("--version") {
            return Ok(ArgumentsRequest::Version);
        } else if arg.eq("-v") || arg.eq_ignore_ascii_case("--verbose") {
            result.verbose = true;
        } else if arg.eq("-s") || arg.eq_ignore_ascii_case("--silent") {
            result.silent = true;
        } else if arg.eq("-x") || arg.eq_ignore_ascii_case("--host") {
            parse_socket_arg(&mut result.server_address, arg, args.next(), DEFAULT_SANDSTORM_PORT).map_err(ArgumentsError::HostError)?;
        } else if arg.eq("-S") || arg.eq_ignore_ascii_case("--shutdown") {
            result.requests.push(CommandRequest::Shutdown);
        } else if arg.eq("-c") || arg.eq_ignore_ascii_case("--credentials") {
            parse_credentials(&mut result.login_credentials, arg, args.next())?;
        } else if arg.eq("-l") || arg.eq_ignore_ascii_case("--list-socks5") {
            result.requests.push(CommandRequest::ListSocks5Sockets);
        } else if arg.eq("-k") || arg.eq_ignore_ascii_case("--add-socks5") {
            parse_socket_arg(&mut tmp_sockets_vec, arg, args.next(), DEFAULT_SOCKS5_PORT).map_err(ArgumentsError::AddSocks5Error)?;
            for socket in &tmp_sockets_vec {
                result.requests.push(CommandRequest::AddSocks5Socket(*socket));
            }
            tmp_sockets_vec.clear();
        } else if arg.eq("-r") || arg.eq_ignore_ascii_case("--remove-socks5") {
            parse_socket_arg(&mut tmp_sockets_vec, arg, args.next(), DEFAULT_SOCKS5_PORT).map_err(ArgumentsError::RemoveSocks5Error)?;
            for socket in &tmp_sockets_vec {
                result.requests.push(CommandRequest::RemoveSocks5Socket(*socket));
            }
            tmp_sockets_vec.clear();
        } else if arg.eq("-L") || arg.eq_ignore_ascii_case("--list-sandstr") {
            result.requests.push(CommandRequest::ListSandstormSockets);
        } else if arg.eq("-K") || arg.eq_ignore_ascii_case("--add-sandstr") {
            parse_socket_arg(&mut tmp_sockets_vec, arg, args.next(), DEFAULT_SANDSTORM_PORT).map_err(ArgumentsError::AddSandstormError)?;
            for socket in &tmp_sockets_vec {
                result.requests.push(CommandRequest::AddSandstormSocket(*socket));
            }
            tmp_sockets_vec.clear();
        } else if arg.eq("-R") || arg.eq_ignore_ascii_case("--remove-sandstr") {
            parse_socket_arg(&mut tmp_sockets_vec, arg, args.next(), DEFAULT_SANDSTORM_PORT)
                .map_err(ArgumentsError::RemoveSandstormError)?;
            for socket in &tmp_sockets_vec {
                result.requests.push(CommandRequest::RemoveSandstormSocket(*socket));
            }
            tmp_sockets_vec.clear();
        } else if arg.eq("-t") || arg.eq_ignore_ascii_case("--list-users") {
            result.requests.push(CommandRequest::ListUsers);
        } else if arg.eq("-u") || arg.eq_ignore_ascii_case("--add-user") {
            let (username, password, role) = parse_add_user(arg, args.next())?;
            result.requests.push(CommandRequest::AddUser(username, password, role));
        } else if arg.eq("-p") || arg.eq_ignore_ascii_case("--update-user") {
            let (username, maybe_password, maybe_role) = parse_update_user(arg, args.next())?;
            result
                .requests
                .push(CommandRequest::UpdateUser(username, maybe_password, maybe_role));
        } else if arg.eq("-d") || arg.eq_ignore_ascii_case("--delete-user") {
            let username = parse_delete_user(arg, args.next())?;
            result.requests.push(CommandRequest::DeleteUser(username));
        } else if arg.eq("-z") || arg.eq_ignore_ascii_case("--list-auth") {
            result.requests.push(CommandRequest::ListAuthMethods);
        } else if arg.eq("-A") || arg.eq_ignore_ascii_case("--auth-enable") {
            let auth_method = parse_auth_arg(arg, args.next())?;
            result.requests.push(CommandRequest::ToggleAuthMethod(auth_method, true));
        } else if arg.eq("-a") || arg.eq_ignore_ascii_case("--auth-disable") {
            let auth_method = parse_auth_arg(arg, args.next())?;
            result.requests.push(CommandRequest::ToggleAuthMethod(auth_method, false));
        } else if arg.eq("-m") || arg.eq_ignore_ascii_case("--get-metrics") {
            result.requests.push(CommandRequest::GetMetrics);
        } else if arg.eq("-B") || arg.eq_ignore_ascii_case("--get-buffer-size") {
            result.requests.push(CommandRequest::GetBufferSize);
        } else if arg.eq("-b") || arg.eq_ignore_ascii_case("--set-buffer-size") {
            let buffer_size = parse_buffer_size_arg(arg, args.next())?;
            result.requests.push(CommandRequest::SetBufferSize(buffer_size));
        } else if arg.eq("-w") || arg.eq_ignore_ascii_case("--meow") {
            result.requests.push(CommandRequest::Meow);
        } else if arg.eq("-o") || arg.eq_ignore_ascii_case("--output-logs") {
            if result.interactive {
                return Err(ArgumentsError::CantMixOutputAndInteractive);
            }
            result.output_logs = true;
        } else if arg.eq("-i") || arg.eq_ignore_ascii_case("--interactive") {
            if result.output_logs {
                return Err(ArgumentsError::CantMixOutputAndInteractive);
            }
            result.interactive = true;
        } else {
            return Err(ArgumentsError::UnknownArgument(arg));
        }
    }

    if result.login_credentials.0.is_empty() {
        result.login_credentials = match parse_env_credentials()? {
            Some(creds) => creds,
            None => return Err(ArgumentsError::NoCredentialsSpecified),
        }
    }

    result.fill_empty_fields_with_defaults();
    Ok(ArgumentsRequest::Run(result))
}
