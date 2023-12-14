//! A simple user management system that stores users with username, password, and role, with the
//! ability to load and save this information from disk using a custom human-readable file format.
//!
//! Note that no information is encrypted nor hashed, passwords are stored in plain text. This is
//! not intended to be a secure system (after all, socks5 credentials are transmitted through the
//! network in plain text).
//!
//! The system is used through the `UserManager` type, which can be asynchronously created from the
//! `UserManager::from` family of methods.
//!
//! The file format used for persistence is very simple, each line consists of a user, where the
//! first character of the line specifies the role ('#' for regular user and '@' for admin),
//! followed by the username, followed by a colon ':', followed by the password until the end of
//! the line (or file).
//!
//! Characters in both the username and password may be escaped with a '\', this allows a username
//! to contain the ':' character. Any character can be escaped. A line may also be a comment by
//! starting with '!'. All lines have whitespaces trimmed at the start and empty lines are ignored.
//!
//! Note: Whitespaces aren't trimmed at the end of a line because that would prevent passwords from
//! ending with whitespaces.
//!
//! An example of a valid file is:
//! ```txt
//! ! This is a comment!
//!
//! ! Our admin Pedro, everybody loves him
//! @pedro:pedrito4321
//!
//! ! Our first user Carlos and his brother Felipe, fucken assholes
//! #carlos:carlitox@33
//! #felipe:mi_hermano_es_un_boludo
//!
//! ! My friend chi:chi, nobody knows why she put a ':' in her name:
//! #chi\:chi:super:secret:password
//! ! Chi:chi's password is "super:secret:password"
//! ```

use std::{fmt, io, path::Path};

use dashmap::{
    mapref::{entry::Entry, one::Ref},
    DashMap,
};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufWriter},
};

use crate::utils::{self, process_lines::ProcessFileLinesError};

const COMMENT_PREFIX_CHAR: char = '!';
const ADMIN_PREFIX_CHAR: char = '@';
const USER_PREFIX_CHAR: char = '#';
const ESCAPE_CHAR: char = '\\';

#[derive(Debug)]
pub struct UserManager {
    users: DashMap<String, UserData>,
}

#[derive(Debug, Clone, Copy)]
pub enum UserRole {
    Admin,
    User,
}

#[derive(Debug)]
pub struct UserData {
    password: String,
    role: UserRole,
}

impl UserData {
    pub fn password(&self) -> &String {
        &self.password
    }

    pub fn role(&self) -> UserRole {
        self.role
    }
}

#[derive(Debug)]
pub enum UserManagerCreationError {
    IO(io::Error),
    InvalidUtf8 { line_number: u32, byte_at: usize },
    LineTooLong { line_number: u32, byte_at: usize },
    ExpectedRoleCharGotEOF(u32, u32),
    InvalidRoleChar(u32, u32, char),
    ExpectedColonGotEOF(u32, u32),
    EmptyUsername(u32, u32),
    EmptyPassword(u32, u32),
    NoUsers,
}

impl From<io::Error> for UserManagerCreationError {
    fn from(value: io::Error) -> Self {
        UserManagerCreationError::IO(value)
    }
}

impl fmt::Display for UserManagerCreationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserManagerCreationError::IO(io_error) => write!(f, "IO error: {}", io_error),
            UserManagerCreationError::InvalidUtf8 { line_number, byte_at } => write!(f, "Invalid UTF-8 at {line_number} byte {byte_at}"),
            UserManagerCreationError::LineTooLong { line_number, byte_at: _ } => write!(f, "Line {line_number} is too long"),
            UserManagerCreationError::ExpectedRoleCharGotEOF(line_number, char_at) => {
                write!(f, "Expected role char, got EOF at {line_number}:{char_at}")
            }
            UserManagerCreationError::InvalidRoleChar(line_number, char_at, char) => write!(
                f,
                "Expected role char ('{ADMIN_PREFIX_CHAR}' or '{USER_PREFIX_CHAR}'), got '{char}' at {line_number}:{char_at}"
            ),
            UserManagerCreationError::ExpectedColonGotEOF(line_number, char_at) => {
                write!(f, "Unexpected EOF (expected colon ':' after name) at {line_number}:{char_at}")
            }
            UserManagerCreationError::EmptyUsername(line_number, char_at) => write!(f, "Empty username field at {line_number}:{char_at}"),
            UserManagerCreationError::EmptyPassword(line_number, char_at) => write!(f, "Empty password field at {line_number}:{char_at}"),
            UserManagerCreationError::NoUsers => write!(f, "No users"),
        }
    }
}

fn parse_line_into_user(s: &str, line_number: u32) -> Result<Option<(String, UserData)>, UserManagerCreationError> {
    let mut chars = s.chars();
    let mut char_at = 1;
    let role_char = chars
        .next()
        .ok_or(UserManagerCreationError::ExpectedRoleCharGotEOF(line_number, char_at))?;
    let role = match role_char {
        COMMENT_PREFIX_CHAR => return Ok(None),
        ADMIN_PREFIX_CHAR => UserRole::Admin,
        USER_PREFIX_CHAR => UserRole::User,
        _ => return Err(UserManagerCreationError::InvalidRoleChar(line_number, char_at, role_char)),
    };

    let mut username = String::new();
    let mut escape_next = false;
    loop {
        let next_char = chars
            .next()
            .ok_or(UserManagerCreationError::ExpectedColonGotEOF(line_number, char_at))?;
        char_at += 1;

        if escape_next {
            username.push(next_char);
            escape_next = false;
        } else if next_char == ESCAPE_CHAR {
            escape_next = true;
        } else if next_char == ':' {
            break;
        } else {
            username.push(next_char);
        }
    }

    if username.is_empty() {
        return Err(UserManagerCreationError::EmptyUsername(line_number, char_at));
    }

    let mut password = String::new();
    let mut escape_next = false;
    for next_char in chars {
        char_at += 1;
        if escape_next {
            password.push(next_char);
            escape_next = false;
        } else if next_char == ESCAPE_CHAR {
            escape_next = true;
        } else {
            password.push(next_char);
        }
    }

    if password.is_empty() {
        return Err(UserManagerCreationError::EmptyPassword(line_number, char_at));
    }

    Ok(Some((username, UserData { password, role })))
}

impl UserManager {
    pub fn new() -> UserManager {
        UserManager { users: DashMap::new() }
    }

    pub async fn from<T>(reader: &mut T) -> Result<UserManager, UserManagerCreationError>
    where
        T: AsyncRead + Unpin + ?Sized,
    {
        let users = DashMap::new();

        let result = utils::process_lines::process_lines_utf8(reader, |s, line_number| {
            let s = s.trim_start();
            if !s.is_empty() {
                if let Some((username, user)) = parse_line_into_user(s, line_number)? {
                    users.insert(username, user);
                }
            }

            Ok::<(), UserManagerCreationError>(())
        })
        .await;

        if let Err(error) = result {
            return Err(match error {
                ProcessFileLinesError::IO(io_error) => UserManagerCreationError::IO(io_error),
                ProcessFileLinesError::InvalidUtf8 { line_number, byte_at } => {
                    UserManagerCreationError::InvalidUtf8 { line_number, byte_at }
                }
                ProcessFileLinesError::LineTooLong { line_number, byte_at } => {
                    UserManagerCreationError::LineTooLong { line_number, byte_at }
                }
                ProcessFileLinesError::Cancelled(_, internal_error) => internal_error,
            });
        }

        if users.is_empty() {
            return Err(UserManagerCreationError::NoUsers);
        }

        Ok(UserManager { users })
    }

    pub async fn from_file<F: AsRef<Path>>(filename: F) -> Result<UserManager, UserManagerCreationError> {
        let mut file = File::open(filename).await?;
        UserManager::from(&mut file).await
    }

    pub async fn save_to<T>(&self, writer: &mut T) -> Result<(), io::Error>
    where
        T: AsyncWrite + Unpin + ?Sized,
    {
        let mut is_first = true;
        for ele in self.users.iter() {
            if !is_first {
                writer.write_u8(b'\n').await?;
            } else {
                is_first = false;
            }

            let role_char = match ele.role {
                UserRole::Admin => ADMIN_PREFIX_CHAR as u8,
                UserRole::User => USER_PREFIX_CHAR as u8,
            };
            writer.write_u8(role_char).await?;

            for &c in ele.key().as_bytes() {
                if c == b'\\' || c == b':' {
                    writer.write_u8(b'\\').await?;
                }
                writer.write_u8(c).await?;
            }

            writer.write_u8(b':').await?;
            writer.write_all(ele.password.as_bytes()).await?;
        }

        Ok(())
    }

    pub async fn save_to_file<F: AsRef<Path>>(&self, filename: F) -> Result<(), io::Error> {
        let file = File::create(filename).await?;
        let mut writer = BufWriter::new(file);
        self.save_to(&mut writer).await?;
        writer.flush().await
    }

    pub fn get(&self, username: &str) -> Option<Ref<'_, String, UserData>> {
        self.users.get(username)
    }

    pub fn insert(&self, username: String, password: String, role: UserRole) -> bool {
        let entry = self.users.entry(username);
        if let Entry::Occupied(_) = entry {
            return false;
        }

        entry.insert(UserData { password, role });
        true
    }
}
