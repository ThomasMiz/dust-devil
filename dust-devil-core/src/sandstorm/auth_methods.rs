use std::io::Error;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    serialize::{ByteRead, ByteWrite, SmallReadList, SmallWriteList},
    socks5::AuthMethod,
};

use super::SandstormCommandType;

/// A Sandstorm list auth methods request.
pub struct ListAuthMethodsRequest;

/// A Sandstorm list auth methods response.
pub struct ListAuthMethodsResponse(
    /// The list of authentication methods returned by the server.
    pub Vec<(AuthMethod, bool)>,
);

/// A borrowed version of [`ListAuthMethodsResponse`].
pub struct ListAuthMethodsResponseRef<'a>(
    /// The list of authentication methods returned by the server.
    &'a [(AuthMethod, bool)],
);

impl ListAuthMethodsResponse {
    pub fn as_ref(&self) -> ListAuthMethodsResponseRef {
        ListAuthMethodsResponseRef(&self.0)
    }
}

impl ByteRead for ListAuthMethodsRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_reader: &mut R) -> Result<Self, Error> {
        Ok(Self)
    }
}

impl ByteWrite for ListAuthMethodsRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        SandstormCommandType::ListAuthMethods.write(writer).await
    }
}

impl ByteRead for ListAuthMethodsResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(<SmallReadList<(AuthMethod, bool)> as ByteRead>::read(reader).await?.0))
    }
}

impl ByteWrite for ListAuthMethodsResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl<'a> ByteWrite for ListAuthMethodsResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::ListAuthMethods, SmallWriteList(self.0)).write(writer).await
    }
}

/// A Sandstorm toggle auth method request.
pub struct ToggleAuthMethodRequest(
    /// The authentication method to alter.
    pub AuthMethod,
    /// The desired state for the authentication method (`true` = enabled, `false` = disabled).
    pub bool,
);

// A Sandstorm toggle auth method response.
pub struct ToggleAuthMethodResponse(
    /// Whether the operation succeeded.
    pub bool,
);

impl ByteRead for ToggleAuthMethodRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(AuthMethod::read(reader).await?, bool::read(reader).await?))
    }
}

impl ByteWrite for ToggleAuthMethodRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::ToggleAuthMethod, self.0, self.1).write(writer).await
    }
}

impl ByteRead for ToggleAuthMethodResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        Ok(Self(bool::read(reader).await?))
    }
}

impl ByteWrite for ToggleAuthMethodResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (SandstormCommandType::ToggleAuthMethod, self.0).write(writer).await
    }
}
