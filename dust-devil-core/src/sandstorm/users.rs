use std::io::{self, ErrorKind};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    serialize::{ByteRead, ByteWrite, SmallReadString, SmallWriteString},
    users::UserRole,
};

use super::{SandstormCommandType, U8ReprEnum};

pub struct ListUsersRequest;
pub struct ListUsersResponse(pub Vec<(String, UserRole)>);
pub struct ListUsersResponseRef<'a>(pub &'a [(String, UserRole)]);

impl ListUsersResponse {
    pub fn as_ref(&self) -> ListUsersResponseRef {
        ListUsersResponseRef(self.0.as_slice())
    }
}

impl ByteRead for ListUsersRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self)
    }
}

impl ByteWrite for ListUsersRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        SandstormCommandType::ListUsers.write(writer).await
    }
}

impl ByteRead for ListUsersResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(<Vec<(String, UserRole)> as ByteRead>::read(reader).await?))
    }
}

impl ByteWrite for ListUsersResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for ListUsersResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::ListUsers, self.0).write(writer).await
    }
}

pub struct AddUserRequest(pub String, pub String, pub UserRole);
pub struct AddUserRequestRef<'a>(pub &'a str, pub &'a str, pub UserRole);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddUserResponse {
    Ok = 0x00,
    AlreadyExists = 0x01,
    InvalidValues = 0x02,
}

impl AddUserRequest {
    pub fn as_ref(&self) -> AddUserRequestRef {
        AddUserRequestRef(&self.0, &self.1, self.2)
    }
}

impl U8ReprEnum for AddUserResponse {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(AddUserResponse::Ok),
            0x01 => Some(AddUserResponse::AlreadyExists),
            0x02 => Some(AddUserResponse::InvalidValues),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteRead for AddUserRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(
            SmallReadString::read(reader).await?.0,
            SmallReadString::read(reader).await?.0,
            UserRole::read(reader).await?,
        ))
    }
}

impl ByteWrite for AddUserRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for AddUserRequestRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        let tuple = (
            SandstormCommandType::AddUser,
            SmallWriteString(self.0),
            SmallWriteString(self.1),
            self.2,
        );
        tuple.write(writer).await
    }
}

impl ByteRead for AddUserResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid AddUserResponse type byte")),
        }
    }
}

impl ByteWrite for AddUserResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::AddUser, self.into_u8()).write(writer).await
    }
}

pub struct UpdateUserRequest(pub String, pub Option<String>, pub Option<UserRole>);
pub struct UpdateUserRequestRef<'a>(pub &'a str, pub Option<&'a str>, pub Option<UserRole>);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateUserResponse {
    Ok = 0x00,
    UserNotFound = 0x01,
    CannotDeleteOnlyAdmin = 0x02,
    NothingWasRequested = 0x03,
}

impl UpdateUserRequest {
    pub fn as_ref(&self) -> UpdateUserRequestRef {
        UpdateUserRequestRef(&self.0, self.1.as_deref(), self.2)
    }
}

impl U8ReprEnum for UpdateUserResponse {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Ok),
            0x01 => Some(Self::UserNotFound),
            0x02 => Some(Self::CannotDeleteOnlyAdmin),
            0x03 => Some(Self::NothingWasRequested),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteRead for UpdateUserRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(
            SmallReadString::read(reader).await?.0,
            <Option<SmallReadString> as ByteRead>::read(reader).await?.map(|s| s.0),
            <Option<UserRole> as ByteRead>::read(reader).await?,
        ))
    }
}

impl ByteWrite for UpdateUserRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        let tuple = (
            SandstormCommandType::UpdateUser,
            SmallWriteString(&self.0),
            self.1.as_ref().map(|s| SmallWriteString(s)),
            self.2,
        );
        tuple.write(writer).await
    }
}

impl<'a> ByteWrite for UpdateUserRequestRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        let tuple = (
            SandstormCommandType::UpdateUser,
            SmallWriteString(self.0),
            self.1.map(SmallWriteString),
            self.2,
        );
        tuple.write(writer).await
    }
}

impl ByteRead for UpdateUserResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid UpdateUserResponse type byte")),
        }
    }
}

impl ByteWrite for UpdateUserResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::UpdateUser, self.into_u8()).write(writer).await
    }
}

pub struct DeleteUserRequest(pub String);
pub struct DeleteUserRequestRef<'a>(pub &'a str);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteUserResponse {
    Ok = 0x00,
    UserNotFound = 0x01,
    CannotDeleteOnlyAdmin = 0x02,
}

impl DeleteUserRequest {
    pub fn as_ref(&self) -> DeleteUserRequestRef {
        DeleteUserRequestRef(&self.0)
    }
}

impl U8ReprEnum for DeleteUserResponse {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Ok),
            0x01 => Some(Self::UserNotFound),
            0x02 => Some(Self::CannotDeleteOnlyAdmin),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteRead for DeleteUserRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(DeleteUserRequest(SmallReadString::read(reader).await?.0))
    }
}

impl ByteWrite for DeleteUserRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for DeleteUserRequestRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::DeleteUser, SmallWriteString(self.0)).write(writer).await
    }
}

impl ByteRead for DeleteUserResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid UpdateUserResponse type byte")),
        }
    }
}

impl ByteWrite for DeleteUserResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::DeleteUser, self.into_u8()).write(writer).await
    }
}
