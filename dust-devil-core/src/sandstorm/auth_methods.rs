use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    serialize::{ByteRead, ByteWrite, SmallReadList, SmallWriteList},
    socks5::AuthMethod,
};

use super::SandstormCommandType;

pub struct ListAuthMethodsRequest;
pub struct ListAuthMethodsResponse(pub Vec<(AuthMethod, bool)>);
pub struct ListAuthMethodsResponseRef<'a>(&'a [(AuthMethod, bool)]);

impl ListAuthMethodsResponse {
    pub fn as_ref(&self) -> ListAuthMethodsResponseRef {
        ListAuthMethodsResponseRef(&self.0)
    }
}

impl ByteRead for ListAuthMethodsRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self)
    }
}

impl ByteWrite for ListAuthMethodsRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        SandstormCommandType::ListAuthMethods.write(writer).await
    }
}

impl ByteRead for ListAuthMethodsResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(<SmallReadList<(AuthMethod, bool)> as ByteRead>::read(reader).await?.0))
    }
}

impl ByteWrite for ListAuthMethodsResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for ListAuthMethodsResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::ListAuthMethods, SmallWriteList(self.0)).write(writer).await
    }
}

pub struct ToggleAuthMethodRequest(pub AuthMethod, pub bool);
pub struct ToggleAuthMethodResponse(pub bool);

impl ByteRead for ToggleAuthMethodRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(AuthMethod::read(reader).await?, bool::read(reader).await?))
    }
}

impl ByteWrite for ToggleAuthMethodRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::ToggleAuthMethod, self.0, self.1).write(writer).await
    }
}

impl ByteRead for ToggleAuthMethodResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(Self(bool::read(reader).await?))
    }
}

impl ByteWrite for ToggleAuthMethodResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (SandstormCommandType::ToggleAuthMethod, self.0).write(writer).await
    }
}
