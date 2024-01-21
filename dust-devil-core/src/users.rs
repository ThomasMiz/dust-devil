//! Constants and types for dealing with Sandstorm users, as well as implementations of
//! [`ByteRead`] and [`ByteWrite`] for these types.

use std::{
    fmt,
    io::{Error, ErrorKind},
};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    serialize::{ByteRead, ByteWrite},
    u8_repr_enum::U8ReprEnum,
};

/// A character used to write comments on the users file.
pub const COMMENT_PREFIX_CHAR: char = '!';

/// A symbol character that identifies the admin user role.
pub const ADMIN_PREFIX_CHAR: char = '@';

/// A symbol character that identifies the regular user role.
pub const REGULAR_PREFIX_CHAR: char = '#';

/// A character used for escape sequences when specifying users.
pub const ESCAPE_CHAR: char = '\\';

/// The default user username.
pub const DEFAULT_USER_USERNAME: &str = "admin";

/// The default user password.
pub const DEFAULT_USER_PASSWORD: &str = "admin";

/// The roles a user can take.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRole {
    Admin = 0x01,
    Regular = 0x02,
}

impl UserRole {
    pub fn into_role_char(self) -> char {
        match self {
            Self::Admin => ADMIN_PREFIX_CHAR,
            Self::Regular => REGULAR_PREFIX_CHAR,
        }
    }
}

impl U8ReprEnum for UserRole {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Admin),
            0x02 => Some(Self::Regular),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteWrite for UserRole {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.into_u8().write(writer).await
    }
}

impl ByteRead for UserRole {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match UserRole::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid UserRole type byte")),
        }
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::Regular => write!(f, "regular"),
        }
    }
}

/// Errors that can occur when the server loads a users file.
#[derive(Debug)]
pub enum UsersLoadingError {
    IO(Error),
    InvalidUtf8 { line_number: u32, byte_at: u64 },
    LineTooLong { line_number: u32, byte_at: u64 },
    ExpectedRoleCharGotEOF(u32, u32),
    InvalidRoleChar(u32, u32, char),
    ExpectedColonGotEOF(u32, u32),
    EmptyUsername(u32, u32),
    UsernameTooLong(u32, u32),
    EmptyPassword(u32, u32),
    PasswordTooLong(u32, u32),
    NoUsers,
}

impl PartialEq for UsersLoadingError {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::IO(io_err) => {
                if let Self::IO(other_err) = other {
                    io_err.kind() == other_err.kind()
                } else {
                    false
                }
            }
            Self::InvalidUtf8 { line_number, byte_at } => {
                matches!(other, Self::InvalidUtf8 { line_number: a2, byte_at: b2 } if (line_number, byte_at) == (a2, b2))
            }
            Self::LineTooLong { line_number, byte_at } => {
                matches!(other, Self::LineTooLong { line_number: a2, byte_at: b2 } if (line_number, byte_at) == (a2, b2))
            }
            Self::ExpectedRoleCharGotEOF(a, b) => matches!(other, Self::ExpectedRoleCharGotEOF(a2, b2) if (a, b) == (a2, b2)),
            Self::InvalidRoleChar(a, b, c) => matches!(other, Self::InvalidRoleChar(a2, b2, c2) if (a, b, c) == (a2, b2, c2)),
            Self::ExpectedColonGotEOF(a, b) => matches!(other, Self::ExpectedColonGotEOF(a2, b2) if (a, b) == (a2, b2)),
            Self::EmptyUsername(a, b) => matches!(other, Self::EmptyUsername(a2, b2) if (a, b) == (a2, b2)),
            Self::UsernameTooLong(a, b) => matches!(other, Self::UsernameTooLong(a2, b2) if (a, b) == (a2, b2)),
            Self::EmptyPassword(a, b) => matches!(other, Self::EmptyPassword(a2, b2) if (a, b) == (a2, b2)),
            Self::PasswordTooLong(a, b) => matches!(other, Self::PasswordTooLong(a2, b2) if (a, b) == (a2, b2)),
            Self::NoUsers => matches!(other, Self::NoUsers),
        }
    }
}

impl From<Error> for UsersLoadingError {
    fn from(value: Error) -> Self {
        UsersLoadingError::IO(value)
    }
}

impl fmt::Display for UsersLoadingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsersLoadingError::IO(io_error) => write!(f, "IO error: {io_error}"),
            UsersLoadingError::InvalidUtf8 { line_number, byte_at } => write!(f, "Invalid UTF-8 at {line_number} byte {byte_at}"),
            UsersLoadingError::LineTooLong { line_number, byte_at: _ } => write!(f, "Line {line_number} is too long"),
            UsersLoadingError::ExpectedRoleCharGotEOF(line_number, char_at) => {
                write!(f, "Expected role char, got EOF at {line_number}:{char_at}")
            }
            UsersLoadingError::InvalidRoleChar(line_number, char_at, char) => write!(
                f,
                "Expected role char ('{ADMIN_PREFIX_CHAR}' or '{REGULAR_PREFIX_CHAR}'), got '{char}' at {line_number}:{char_at}"
            ),
            UsersLoadingError::ExpectedColonGotEOF(line_number, char_at) => {
                write!(f, "Unexpected EOF (expected colon ':' after name) at {line_number}:{char_at}")
            }
            UsersLoadingError::EmptyUsername(line_number, char_at) => write!(f, "Empty username field at {line_number}:{char_at}"),
            UsersLoadingError::UsernameTooLong(line_number, char_at) => write!(f, "Username too long at {line_number}:{char_at}"),
            UsersLoadingError::EmptyPassword(line_number, char_at) => write!(f, "Empty password field at {line_number}:{char_at}"),
            UsersLoadingError::PasswordTooLong(line_number, char_at) => write!(f, "Password too long at {line_number}:{char_at}"),
            UsersLoadingError::NoUsers => write!(f, "No users"),
        }
    }
}

impl ByteWrite for UsersLoadingError {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        match self {
            UsersLoadingError::IO(io_error) => (1u8, io_error).write(writer).await,
            UsersLoadingError::InvalidUtf8 { line_number, byte_at } => (2u8, line_number, byte_at).write(writer).await,
            UsersLoadingError::LineTooLong { line_number, byte_at } => (3u8, line_number, byte_at).write(writer).await,
            UsersLoadingError::ExpectedRoleCharGotEOF(line_number, char_at) => (4u8, line_number, char_at).write(writer).await,
            UsersLoadingError::InvalidRoleChar(line_number, char_at, char) => (5u8, line_number, char_at, *char).write(writer).await,
            UsersLoadingError::ExpectedColonGotEOF(line_number, char_at) => (6u8, line_number, char_at).write(writer).await,
            UsersLoadingError::EmptyUsername(line_number, char_at) => (7u8, line_number, char_at).write(writer).await,
            UsersLoadingError::UsernameTooLong(line_number, char_at) => (8u8, line_number, char_at).write(writer).await,
            UsersLoadingError::EmptyPassword(line_number, char_at) => (9u8, line_number, char_at).write(writer).await,
            UsersLoadingError::PasswordTooLong(line_number, char_at) => (10u8, line_number, char_at).write(writer).await,
            UsersLoadingError::NoUsers => 11u8.write(writer).await,
        }
    }
}

impl ByteRead for UsersLoadingError {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let t = u8::read(reader).await?;

        match t {
            1 => Ok(UsersLoadingError::IO(Error::read(reader).await?)),
            2 => Ok(UsersLoadingError::InvalidUtf8 {
                line_number: u32::read(reader).await?,
                byte_at: u64::read(reader).await?,
            }),
            3 => Ok(UsersLoadingError::LineTooLong {
                line_number: u32::read(reader).await?,
                byte_at: u64::read(reader).await?,
            }),
            4 => Ok(UsersLoadingError::ExpectedRoleCharGotEOF(
                u32::read(reader).await?,
                u32::read(reader).await?,
            )),
            5 => Ok(UsersLoadingError::InvalidRoleChar(
                u32::read(reader).await?,
                u32::read(reader).await?,
                char::read(reader).await?,
            )),
            6 => Ok(UsersLoadingError::ExpectedColonGotEOF(
                u32::read(reader).await?,
                u32::read(reader).await?,
            )),
            7 => Ok(UsersLoadingError::EmptyUsername(u32::read(reader).await?, u32::read(reader).await?)),
            8 => Ok(UsersLoadingError::UsernameTooLong(
                u32::read(reader).await?,
                u32::read(reader).await?,
            )),
            9 => Ok(UsersLoadingError::EmptyPassword(u32::read(reader).await?, u32::read(reader).await?)),
            10 => Ok(UsersLoadingError::PasswordTooLong(
                u32::read(reader).await?,
                u32::read(reader).await?,
            )),
            11 => Ok(UsersLoadingError::NoUsers),
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid UsersLoadingError type byte")),
        }
    }
}
