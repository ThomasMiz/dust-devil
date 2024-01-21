use std::io::{Error, ErrorKind};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    serialize::{ByteRead, ByteWrite, SmallReadString, SmallWriteString},
    u8_repr_enum::U8ReprEnum,
};

/// A Sandstorm handshake client request, implicitly version 1.
pub struct SandstormHandshake {
    /// The username of the user to log in as.
    pub username: String,

    /// The password of the user to log in as.
    pub password: String,
}

/// A borrowed version of [`SandstormHandshake`].
pub struct SandstormHandshakeRef<'a> {
    /// The username of the user to log in as.
    pub username: &'a str,

    /// The password of the user to log in as.
    pub password: &'a str,
}

/// An error from reading a [`SandstormHandshake`].
pub enum ParseHandshakeError {
    /// Indicates that the client requested an invalid or unsupported Sandstorm version.
    InvalidVersion(u8),

    /// Indicates that an IO error ocurred while reading the handshake.
    IO(Error),
}

impl From<Error> for ParseHandshakeError {
    fn from(value: Error) -> Self {
        ParseHandshakeError::IO(value)
    }
}

impl SandstormHandshake {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }

    pub fn as_ref(&self) -> SandstormHandshakeRef {
        SandstormHandshakeRef {
            username: &self.username,
            password: &self.password,
        }
    }

    pub async fn read_with_version_check<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, ParseHandshakeError> {
        let version = u8::read(reader).await?;
        if version != 1 {
            Err(ParseHandshakeError::InvalidVersion(version))
        } else {
            Ok(Self {
                username: SmallReadString::read(reader).await?.0,
                password: SmallReadString::read(reader).await?.0,
            })
        }
    }
}

impl<'a> SandstormHandshakeRef<'a> {
    pub fn new(username: &'a str, password: &'a str) -> Self {
        Self { username, password }
    }
}

impl ByteRead for SandstormHandshake {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match Self::read_with_version_check(reader).await {
            Ok(value) => Ok(value),
            Err(ParseHandshakeError::InvalidVersion(version)) => {
                Err(Error::new(ErrorKind::InvalidInput, format!("Invalid Sandstorm version: {version}")))
            }
            Err(ParseHandshakeError::IO(error)) => Err(error),
        }
    }
}

impl ByteWrite for SandstormHandshake {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for SandstormHandshakeRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (0x01u8, SmallWriteString(self.username), SmallWriteString(self.password))
            .write(writer)
            .await
    }
}

/// A Sandstorm handshake server response, indicating the result of the handshake.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandstormHandshakeStatus {
    /// The handshake succeeded and access was granted.
    Ok = 0x00,

    /// The client requested an invalid or unsupported Sandstorm version.
    UnsupportedVersion = 0x01,

    /// The client specified invalid username or password.
    InvalidUsernameOrPassword = 0x02,

    /// The client successfully logged in, but the user doesn't have monitoring permissions.
    PermissionDenied = 0x03,

    /// Unspecified error.
    UnspecifiedError = 0xFF,
}

impl U8ReprEnum for SandstormHandshakeStatus {
    fn from_u8(value: u8) -> Option<SandstormHandshakeStatus> {
        match value {
            0x00 => Some(SandstormHandshakeStatus::Ok),
            0x01 => Some(SandstormHandshakeStatus::UnsupportedVersion),
            0x02 => Some(SandstormHandshakeStatus::InvalidUsernameOrPassword),
            0x03 => Some(SandstormHandshakeStatus::PermissionDenied),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteRead for SandstormHandshakeStatus {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(value) => Ok(value),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid SandstormHandshakeStatus type byte")),
        }
    }
}
impl ByteWrite for SandstormHandshakeStatus {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.into_u8().write(writer).await
    }
}
