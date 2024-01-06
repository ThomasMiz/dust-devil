use std::{fmt, io};

pub const COMMENT_PREFIX_CHAR: char = '!';
pub const ADMIN_PREFIX_CHAR: char = '@';
pub const REGULAR_PREFIX_CHAR: char = '#';
pub const ESCAPE_CHAR: char = '\\';

pub const DEFAULT_USER_USERNAME: &str = "admin";
pub const DEFAULT_USER_PASSWORD: &str = "admin";

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UserRole {
    Admin = 1,
    Regular = 2,
}

impl UserRole {
    pub fn from_u8(value: u8) -> Option<UserRole> {
        match value {
            1 => Some(UserRole::Admin),
            2 => Some(UserRole::Regular),
            _ => None,
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

#[derive(Debug)]
pub enum UsersLoadingError {
    IO(io::Error),
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

impl From<io::Error> for UsersLoadingError {
    fn from(value: io::Error) -> Self {
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
