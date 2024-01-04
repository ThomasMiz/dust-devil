use std::{
    collections::HashMap,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};

use dust_devil_core::users::UserRole;

use crate::{
    args::{BufferSizeErrorType, DEFAULT_SANDSTORM_PORT},
    users::UserData,
};

use crate::args::{
    parse_arguments, ArgumentsError, ArgumentsRequest, AuthToggleErrorType, FileErrorType, ListenErrorType, NewUserErrorType,
    StartupArguments, DEFAULT_SOCKS5_PORT,
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
fn test_verbose() {
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
fn test_silent() {
    let result = args("-s");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            silent: true,
            ..Default::default()
        }))
    );

    let result = args("--silent");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            silent: true,
            ..Default::default()
        }))
    );
}

#[test]
fn test_log_file() {
    let result = args("-o ./some/dir/file.txt");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            log_file: Some("./some/dir/file.txt".to_string()),
            ..Default::default()
        }))
    );

    let result = args("--log-file ./some/dir/file.txt");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            log_file: Some("./some/dir/file.txt".to_string()),
            ..Default::default()
        }))
    );
}

#[test]
fn test_log_file_empty() {
    let result = args_vec(&["-o", ""]);
    assert_eq!(
        result,
        Err(ArgumentsError::LogFileError(FileErrorType::EmptyPath("-o".to_string())))
    );

    let result = args_vec(&["--log-file", ""]);
    assert_eq!(
        result,
        Err(ArgumentsError::LogFileError(FileErrorType::EmptyPath("--log-file".to_string())))
    );
}

#[test]
fn test_log_file_unexpected_end() {
    let result = args("-o");
    assert_eq!(
        result,
        Err(ArgumentsError::LogFileError(FileErrorType::UnexpectedEnd("-o".to_string())))
    );

    let result = args("--log-file");
    assert_eq!(
        result,
        Err(ArgumentsError::LogFileError(FileErrorType::UnexpectedEnd("--log-file".to_string())))
    );
}

#[test]
fn test_log_file_specified_twice() {
    let result = args("-o ./my_log -v --log-file againnnn");
    assert_eq!(
        result,
        Err(ArgumentsError::LogFileError(FileErrorType::AlreadySpecified(
            "--log-file".to_string()
        )))
    );

    let result = args("--log-file ./my_log -v -o againnnn");
    assert_eq!(
        result,
        Err(ArgumentsError::LogFileError(FileErrorType::AlreadySpecified("-o".to_string())))
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
            socks5_bind_sockets: vec![SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), DEFAULT_SOCKS5_PORT))],
            ..Default::default()
        }))
    );

    let result = args("-l 127.0.4.20 -l [fefe::afaf%69420]");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            socks5_bind_sockets: vec![
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 4, 20), DEFAULT_SOCKS5_PORT)),
                SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::new(0xfefe, 0, 0, 0, 0, 0, 0, 0xafaf),
                    DEFAULT_SOCKS5_PORT,
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
        Err(ArgumentsError::Socks5ListenError(ListenErrorType::UnexpectedEnd("-l".to_string())))
    );

    let result = args("--listen");
    assert_eq!(
        result,
        Err(ArgumentsError::Socks5ListenError(ListenErrorType::UnexpectedEnd(
            "--listen".to_string()
        )))
    );
}

#[test]
fn test_listen_bad_format() {
    let result = args("-l 127.420.666.0");
    assert_eq!(
        result,
        Err(ArgumentsError::Socks5ListenError(ListenErrorType::InvalidSocketAddress(
            "-l".to_string(),
            "127.420.666.0".to_string()
        )))
    );

    let result = args("--listen [fafa::fefe:fifi:fofo:fufu]");
    assert_eq!(
        result,
        Err(ArgumentsError::Socks5ListenError(ListenErrorType::InvalidSocketAddress(
            "--listen".to_string(),
            "[fafa::fefe:fifi:fofo:fufu]".to_string()
        )))
    );

    let result = args_vec(&["--listen", "alto chori ameo 🤩🤩"]);
    assert_eq!(
        result,
        Err(ArgumentsError::Socks5ListenError(ListenErrorType::InvalidSocketAddress(
            "--listen".to_string(),
            "alto chori ameo 🤩🤩".to_string()
        )))
    );
}

#[test]
fn test_listen_sandstorm_single() {
    let result = args("-m 9.8.7.6:54321");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            sandstorm_bind_sockets: vec![SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(9, 8, 7, 6), 54321))],
            ..Default::default()
        }))
    );
}

#[test]
fn test_listen_sandstorm_default_port() {
    let result = args("-m 4.3.4.3");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            sandstorm_bind_sockets: vec![SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(4, 3, 4, 3), DEFAULT_SANDSTORM_PORT))],
            ..Default::default()
        }))
    );

    let result = args("-m 69.6.4.20 -m [fefe::bebe%42069]");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            sandstorm_bind_sockets: vec![
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(69, 6, 4, 20), DEFAULT_SANDSTORM_PORT)),
                SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::new(0xfefe, 0, 0, 0, 0, 0, 0, 0xbebe),
                    DEFAULT_SANDSTORM_PORT,
                    0,
                    42069
                )),
            ],
            ..Default::default()
        }))
    );
}

#[test]
fn test_listen_sandstorm_multiple() {
    let result = args("-m [abcd::4f5:2e2e:4321:3ac3%69]:7164 -m 1.2.3.4:56789");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            sandstorm_bind_sockets: vec![
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
fn test_listen_sandstorm_unexpected_end() {
    let result = args("-m");
    assert_eq!(
        result,
        Err(ArgumentsError::SandstormListenError(ListenErrorType::UnexpectedEnd(
            "-m".to_string()
        )))
    );

    let result = args("--management");
    assert_eq!(
        result,
        Err(ArgumentsError::SandstormListenError(ListenErrorType::UnexpectedEnd(
            "--management".to_string()
        )))
    );
}

#[test]
fn test_listen_sandstorm_bad_format() {
    let result = args("-m 127.420.666.0");
    assert_eq!(
        result,
        Err(ArgumentsError::SandstormListenError(ListenErrorType::InvalidSocketAddress(
            "-m".to_string(),
            "127.420.666.0".to_string()
        )))
    );

    let result = args("--management [fafa::fefe:fifi:fofo:fufu]");
    assert_eq!(
        result,
        Err(ArgumentsError::SandstormListenError(ListenErrorType::InvalidSocketAddress(
            "--management".to_string(),
            "[fafa::fefe:fifi:fofo:fufu]".to_string()
        )))
    );

    let result = args_vec(&["--management", "alto chori ameo 🤩🤩"]);
    assert_eq!(
        result,
        Err(ArgumentsError::SandstormListenError(ListenErrorType::InvalidSocketAddress(
            "--management".to_string(),
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
        Err(ArgumentsError::UsersFileError(FileErrorType::EmptyPath("-U".to_string())))
    );

    let result = args_vec(&["--users-file", ""]);
    assert_eq!(
        result,
        Err(ArgumentsError::UsersFileError(FileErrorType::EmptyPath("--users-file".to_string())))
    );
}

#[test]
fn test_users_file_unexpected_end() {
    let result = args("-U");
    assert_eq!(
        result,
        Err(ArgumentsError::UsersFileError(FileErrorType::UnexpectedEnd("-U".to_string())))
    );

    let result = args("--users-file");
    assert_eq!(
        result,
        Err(ArgumentsError::UsersFileError(FileErrorType::UnexpectedEnd(
            "--users-file".to_string()
        )))
    );
}

#[test]
fn test_users_file_specified_twice() {
    let result = args("-U ./my_users -v --users-file againnnn");
    assert_eq!(
        result,
        Err(ArgumentsError::UsersFileError(FileErrorType::AlreadySpecified(
            "--users-file".to_string()
        )))
    );

    let result = args("--users-file ./my_users -v -U againnnn");
    assert_eq!(
        result,
        Err(ArgumentsError::UsersFileError(FileErrorType::AlreadySpecified("-U".to_string())))
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
fn test_buffer_size_decimal() {
    let result = args("-b 1234");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 1234,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 4321");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 4321,
            ..Default::default()
        }))
    );

    let result = args("-b 43k");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 43 * 1024,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 12K");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 12 * 1024,
            ..Default::default()
        }))
    );

    let result = args("-b 33M");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 33 * 1024 * 1024,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 9m");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 9 * 1024 * 1024,
            ..Default::default()
        }))
    );

    let result = args("-b 3g");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 3 * 1024 * 1024 * 1024,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 1G");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 1024 * 1024 * 1024,
            ..Default::default()
        }))
    );
}

#[test]
fn test_buffer_size_octal() {
    let result = args("-b 0o1234");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0o1234,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 0o4321");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0o4321,
            ..Default::default()
        }))
    );

    let result = args("-b 0o43k");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0o43 * 1024,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 0o12K");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0o12 * 1024,
            ..Default::default()
        }))
    );

    let result = args("-b 0o33M");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0o33 * 1024 * 1024,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 0o11m");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0o11 * 1024 * 1024,
            ..Default::default()
        }))
    );

    let result = args("-b 0o3g");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0o3 * 1024 * 1024 * 1024,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 0o1G");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 1024 * 1024 * 1024,
            ..Default::default()
        }))
    );
}

#[test]
fn test_buffer_size_binary() {
    let result = args("-b 0b10110101101010101");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0b10110101101010101,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 0b111100001010");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0b111100001010,
            ..Default::default()
        }))
    );

    let result = args("-b 0b1110010010101k");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0b1110010010101 * 1024,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 0b10101011K");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0b10101011 * 1024,
            ..Default::default()
        }))
    );

    let result = args("-b 0b101010M");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0b101010 * 1024 * 1024,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 0b11m");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0b11 * 1024 * 1024,
            ..Default::default()
        }))
    );

    let result = args("-b 0b11g");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 0b11 * 1024 * 1024 * 1024,
            ..Default::default()
        }))
    );

    let result = args("--buffer-size 0b1G");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            buffer_size: 1024 * 1024 * 1024,
            ..Default::default()
        }))
    );
}

#[test]
fn test_buffer_size_unexpected_end() {
    let result = args("-b");
    assert_eq!(
        result,
        Err(ArgumentsError::BufferSizeError(BufferSizeErrorType::UnexpectedEnd(
            "-b".to_string()
        )))
    );

    let result = args("--buffer-size");
    assert_eq!(
        result,
        Err(ArgumentsError::BufferSizeError(BufferSizeErrorType::UnexpectedEnd(
            "--buffer-size".to_string()
        )))
    );
}

#[test]
fn test_buffer_size_unknown_suffix() {
    let result = args("--buffer-size 2q");
    assert_eq!(
        result,
        Err(ArgumentsError::BufferSizeError(BufferSizeErrorType::InvalidSize(
            "--buffer-size".to_string(),
            "2q".to_string()
        )))
    );

    let result = args("-b 2W");
    assert_eq!(
        result,
        Err(ArgumentsError::BufferSizeError(BufferSizeErrorType::InvalidSize(
            "-b".to_string(),
            "2W".to_string()
        )))
    );
}

#[test]
fn test_buffer_size_too_big() {
    let result = args("-b 4G");
    assert_eq!(
        result,
        Err(ArgumentsError::BufferSizeError(BufferSizeErrorType::InvalidSize(
            "-b".to_string(),
            "4G".to_string()
        )))
    );

    let result = args("-b 4294967296");
    assert_eq!(
        result,
        Err(ArgumentsError::BufferSizeError(BufferSizeErrorType::InvalidSize(
            "-b".to_string(),
            "4294967296".to_string()
        )))
    );

    let result = args("-b 4096M");
    assert_eq!(
        result,
        Err(ArgumentsError::BufferSizeError(BufferSizeErrorType::InvalidSize(
            "-b".to_string(),
            "4096M".to_string()
        )))
    );

    let result = args("-b 12G");
    assert_eq!(
        result,
        Err(ArgumentsError::BufferSizeError(BufferSizeErrorType::InvalidSize(
            "-b".to_string(),
            "12G".to_string()
        )))
    );
}

#[test]
fn test_unknown_argument() {
    let result = args("-q");
    assert_eq!(result, Err(ArgumentsError::UnknownArgument("-q".to_string())));

    let result = args("croquetas");
    assert_eq!(result, Err(ArgumentsError::UnknownArgument("croquetas".to_string())));

    let result = args("--sacacorchos");
    assert_eq!(result, Err(ArgumentsError::UnknownArgument("--sacacorchos".to_string())));
}

#[test]
fn test_integration1() {
    let result = args(
        "-l 0.0.0.0 -v -u #pedro:pedro -l [::1%6969]:6060 -b 1K -U myfile.txt --silent --auth-disable noauth -u @\\\\so\\:co:tr\\\\oco",
    );
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            socks5_bind_sockets: vec![
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DEFAULT_SOCKS5_PORT)),
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 6060, 0, 6969)),
            ],
            verbose: true,
            silent: true,
            users: usermap(&[("pedro", "pedro", UserRole::Regular), ("\\so:co", "tr\\oco", UserRole::Admin),]),
            users_file: "myfile.txt".to_string(),
            buffer_size: 1024,
            no_auth_enabled: false,
            ..Default::default()
        }))
    );
}

#[test]
fn test_integration2() {
    let result = args("-U picante.txt -m 4.3.2.1 --auth-enable noauth -u juan:carlos -v -u #carlos:juan --log-file myfile.txt --auth-disable userpass -l 1.2.3.4:5678");
    assert_eq!(
        result,
        Ok(ArgumentsRequest::Run(StartupArguments {
            users_file: "picante.txt".to_string(),
            sandstorm_bind_sockets: vec![SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(4, 3, 2, 1), DEFAULT_SANDSTORM_PORT))],
            no_auth_enabled: true,
            userpass_auth_enabled: false,
            users: usermap(&[("juan", "carlos", UserRole::Regular), ("carlos", "juan", UserRole::Regular),]),
            log_file: Some("myfile.txt".to_string()),
            verbose: true,
            socks5_bind_sockets: vec![SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 5678))],
            ..Default::default()
        }))
    );
}
