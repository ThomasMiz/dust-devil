use std::{
    io::{self, ErrorKind},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    logging::{LogEvent, LogEventType},
    sandstorm::{AddUserResponse, DeleteUserResponse, SandstormCommandType, UpdateUserResponse},
    socks5::{AuthMethod, SocksRequest, SocksRequestAddress},
    users::{UserRole, UsersLoadingError},
};

#[allow(async_fn_in_trait)]
pub trait ByteWrite {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error>;
}

#[allow(async_fn_in_trait)]
pub trait ByteRead: Sized {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error>;
}

impl ByteWrite for () {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, _: &mut W) -> Result<(), io::Error> {
        Ok(())
    }
}

impl ByteRead for () {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_: &mut R) -> Result<Self, io::Error> {
        Ok(())
    }
}

impl ByteWrite for bool {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8(*self as u8).await
    }
}

impl ByteRead for bool {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(reader.read_u8().await? != 0)
    }
}

impl ByteWrite for u8 {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8(*self).await
    }
}

impl ByteRead for u8 {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        reader.read_u8().await
    }
}

impl ByteWrite for u16 {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u16(*self).await
    }
}

impl ByteRead for u16 {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        reader.read_u16().await
    }
}

impl ByteWrite for u32 {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u32(*self).await
    }
}

impl ByteRead for u32 {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        reader.read_u32().await
    }
}

impl ByteWrite for u64 {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u64(*self).await
    }
}

impl ByteRead for u64 {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        reader.read_u64().await
    }
}

impl ByteWrite for char {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u32(*self as u32).await
    }
}

impl ByteRead for char {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let c = reader.read_u32().await?;
        char::from_u32(c).ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "char is not valid UTF-8"))
    }
}

impl<T0: ByteWrite, T1: ByteWrite> ByteWrite for (T0, T1) {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.0.write(writer).await?;
        self.1.write(writer).await
    }
}

impl<T0: ByteRead, T1: ByteRead> ByteRead for (T0, T1) {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok((T0::read(reader).await?, T1::read(reader).await?))
    }
}

impl<T0: ByteWrite, T1: ByteWrite, T2: ByteWrite> ByteWrite for (T0, T1, T2) {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.0.write(writer).await?;
        self.1.write(writer).await?;
        self.2.write(writer).await
    }
}

impl<T0: ByteRead, T1: ByteRead, T2: ByteRead> ByteRead for (T0, T1, T2) {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok((T0::read(reader).await?, T1::read(reader).await?, T2::read(reader).await?))
    }
}

impl<T0: ByteWrite, T1: ByteWrite, T2: ByteWrite, T3: ByteWrite> ByteWrite for (T0, T1, T2, T3) {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.0.write(writer).await?;
        self.1.write(writer).await?;
        self.2.write(writer).await?;
        self.3.write(writer).await
    }
}

impl<T0: ByteRead, T1: ByteRead, T2: ByteRead, T3: ByteRead> ByteRead for (T0, T1, T2, T3) {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok((
            T0::read(reader).await?,
            T1::read(reader).await?,
            T2::read(reader).await?,
            T3::read(reader).await?,
        ))
    }
}

impl<T0: ByteWrite, T1: ByteWrite, T2: ByteWrite, T3: ByteWrite, T4: ByteWrite> ByteWrite for (T0, T1, T2, T3, T4) {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.0.write(writer).await?;
        self.1.write(writer).await?;
        self.2.write(writer).await?;
        self.3.write(writer).await?;
        self.4.write(writer).await
    }
}

impl<T0: ByteRead, T1: ByteRead, T2: ByteRead, T3: ByteRead, T4: ByteRead> ByteRead for (T0, T1, T2, T3, T4) {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok((
            T0::read(reader).await?,
            T1::read(reader).await?,
            T2::read(reader).await?,
            T3::read(reader).await?,
            T4::read(reader).await?,
        ))
    }
}

impl ByteWrite for Ipv4Addr {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_all(&self.octets()).await
    }
}

impl ByteRead for Ipv4Addr {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let mut octets = [0u8; 4];
        reader.read_exact(&mut octets).await?;
        Ok(octets.into())
    }
}

impl ByteWrite for Ipv6Addr {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_all(&self.octets()).await
    }
}

impl ByteRead for Ipv6Addr {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let mut octets = [0u8; 16];
        reader.read_exact(&mut octets).await?;

        Ok(octets.into())
    }
}

impl ByteWrite for SocketAddrV4 {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.ip().write(writer).await?;
        writer.write_u16(self.port()).await
    }
}

impl ByteRead for SocketAddrV4 {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let mut octets = [0u8; 4];
        reader.read_exact(&mut octets).await?;
        let port = reader.read_u16().await?;

        Ok(SocketAddrV4::new(octets.into(), port))
    }
}

impl ByteWrite for SocketAddrV6 {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.ip().write(writer).await?;
        writer.write_u16(self.port()).await?;
        writer.write_u32(self.flowinfo()).await?;
        writer.write_u32(self.scope_id()).await
    }
}

impl ByteRead for SocketAddrV6 {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let mut octets = [0u8; 16];
        reader.read_exact(&mut octets).await?;
        let port = reader.read_u16().await?;
        let flowinfo = reader.read_u32().await?;
        let scope_id = reader.read_u32().await?;

        Ok(SocketAddrV6::new(octets.into(), port, flowinfo, scope_id))
    }
}

impl ByteWrite for SocketAddr {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            SocketAddr::V4(v4) => {
                writer.write_u8(4).await?;
                v4.write(writer).await
            }
            SocketAddr::V6(v6) => {
                writer.write_u8(6).await?;
                v6.write(writer).await
            }
        }
    }
}

impl ByteRead for SocketAddr {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let addr_type = reader.read_u8().await?;
        match addr_type {
            4 => Ok(SocketAddr::V4(SocketAddrV4::read(reader).await?)),
            6 => Ok(SocketAddr::V6(SocketAddrV6::read(reader).await?)),
            v => Err(io::Error::new(ErrorKind::InvalidData, format!("Invalid socket address type, {v}"))),
        }
    }
}

impl<T: ByteWrite> ByteWrite for Option<T> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Some(value) => {
                writer.write_u8(1).await?;
                value.write(writer).await
            }
            None => writer.write_u8(0).await,
        }
    }
}

impl<T: ByteRead> ByteRead for Option<T> {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let has_value = reader.read_u8().await?;
        match has_value {
            0 => Ok(None),
            _ => Ok(Some(T::read(reader).await?)),
        }
    }
}

impl<T: ByteWrite, E: ByteWrite> ByteWrite for Result<T, E> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Ok(v) => {
                writer.write_u8(1).await?;
                v.write(writer).await
            }
            Err(e) => {
                writer.write_u8(0).await?;
                e.write(writer).await
            }
        }
    }
}

impl<T: ByteRead, E: ByteRead> ByteRead for Result<T, E> {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match reader.read_u8().await? {
            0 => Ok(Err(E::read(reader).await?)),
            _ => Ok(Ok(T::read(reader).await?)),
        }
    }
}

impl ByteWrite for io::Error {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        let kind_id = match self.kind() {
            ErrorKind::NotFound => 1,
            ErrorKind::PermissionDenied => 2,
            ErrorKind::ConnectionRefused => 3,
            ErrorKind::ConnectionReset => 4,
            ErrorKind::ConnectionAborted => 5,
            ErrorKind::NotConnected => 6,
            ErrorKind::AddrInUse => 7,
            ErrorKind::AddrNotAvailable => 8,
            ErrorKind::BrokenPipe => 9,
            ErrorKind::AlreadyExists => 10,
            ErrorKind::WouldBlock => 11,
            ErrorKind::InvalidInput => 12,
            ErrorKind::InvalidData => 13,
            ErrorKind::TimedOut => 14,
            ErrorKind::WriteZero => 15,
            ErrorKind::Interrupted => 16,
            ErrorKind::Unsupported => 17,
            ErrorKind::UnexpectedEof => 18,
            ErrorKind::OutOfMemory => 19,
            ErrorKind::Other => 20,
            _ => 0,
        };

        writer.write_u8(kind_id).await
    }
}

impl ByteRead for io::Error {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let kind_id = reader.read_u8().await?;

        let error_kind = match kind_id {
            1 => ErrorKind::NotFound,
            2 => ErrorKind::PermissionDenied,
            3 => ErrorKind::ConnectionRefused,
            4 => ErrorKind::ConnectionReset,
            5 => ErrorKind::ConnectionAborted,
            6 => ErrorKind::NotConnected,
            7 => ErrorKind::AddrInUse,
            8 => ErrorKind::AddrNotAvailable,
            9 => ErrorKind::BrokenPipe,
            10 => ErrorKind::AlreadyExists,
            11 => ErrorKind::WouldBlock,
            12 => ErrorKind::InvalidInput,
            13 => ErrorKind::InvalidData,
            14 => ErrorKind::TimedOut,
            15 => ErrorKind::WriteZero,
            16 => ErrorKind::Interrupted,
            17 => ErrorKind::Unsupported,
            18 => ErrorKind::UnexpectedEof,
            19 => ErrorKind::OutOfMemory,
            _ => ErrorKind::Other,
        };

        Ok(error_kind.into())
    }
}

impl ByteWrite for &str {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        let bytes = self.as_bytes();
        let len = bytes.len();
        if len > u16::MAX as usize {
            return Err(io::Error::new(ErrorKind::InvalidData, "String is too long (>= 64KB)"));
        }

        let len = len as u16;
        writer.write_u16(len).await?;
        writer.write_all(bytes).await
    }
}

impl ByteWrite for String {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.as_str().write(writer).await
    }
}

impl ByteRead for String {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let len = reader.read_u16().await? as usize;

        let mut s = String::with_capacity(len);
        unsafe {
            // SAFETY: The elements of `v` are initialized by `read_exact`, and then we ensure they are valid UTF-8.
            let v = s.as_mut_vec();
            v.set_len(len);
            reader.read_exact(&mut v[0..len]).await?;
            if std::str::from_utf8(v).is_err() {
                return Err(io::Error::new(ErrorKind::InvalidData, "String is not valid UTF-8"));
            }
        }

        Ok(s)
    }
}

pub struct SmallWriteString<'a>(pub &'a str);

impl<'a> ByteWrite for SmallWriteString<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        let bytes = self.0.as_bytes();
        let len = bytes.len();
        if len > u8::MAX as usize {
            return Err(io::Error::new(ErrorKind::InvalidData, "Small string is too long (>= 256B)"));
        }

        let len = len as u8;
        writer.write_u8(len).await?;
        writer.write_all(bytes).await
    }
}

pub struct SmallReadString(pub String);

impl ByteRead for SmallReadString {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let len = reader.read_u8().await? as usize;

        let mut s = String::with_capacity(len);
        unsafe {
            // SAFETY: The elements of `v` are initialized by `read_exact`, and then we ensure they are valid UTF-8.
            let v = s.as_mut_vec();
            v.set_len(len);
            reader.read_exact(&mut v[0..len]).await?;
            if std::str::from_utf8(v).is_err() {
                return Err(io::Error::new(ErrorKind::InvalidData, "Small string is not valid UTF-8"));
            }
        }

        Ok(SmallReadString(s))
    }
}

impl<T: ByteWrite> ByteWrite for &[T] {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        let len = self.len();
        if len > u16::MAX as usize {
            return Err(io::Error::new(ErrorKind::InvalidData, "List is too long (>= 64K)"));
        }

        let len = len as u16;
        writer.write_u16(len).await?;
        for ele in self.iter() {
            ele.write(writer).await?;
        }

        Ok(())
    }
}

impl<T: ByteRead> ByteRead for Vec<T> {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let len = reader.read_u16().await? as usize;

        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            v.push(T::read(reader).await?);
        }

        Ok(v)
    }
}

pub struct SmallWriteList<'a, T>(pub &'a [T]);

impl<'a, T: ByteWrite> ByteWrite for SmallWriteList<'a, T> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        let len = self.0.len();
        if len > u8::MAX as usize {
            return Err(io::Error::new(ErrorKind::InvalidData, "Small list is too long (>= 256)"));
        }

        let len = len as u8;
        writer.write_u8(len).await?;
        for ele in self.0.iter() {
            ele.write(writer).await?;
        }

        Ok(())
    }
}

pub struct SmallReadList<T>(pub Vec<T>);

impl<T: ByteRead> ByteRead for SmallReadList<T> {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let len = reader.read_u8().await? as usize;

        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            v.push(T::read(reader).await?);
        }

        Ok(SmallReadList(v))
    }
}

impl ByteWrite for UsersLoadingError {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
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
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let t = reader.read_u8().await?;

        match t {
            1 => Ok(UsersLoadingError::IO(io::Error::read(reader).await?)),
            2 => Ok(UsersLoadingError::InvalidUtf8 {
                line_number: reader.read_u32().await?,
                byte_at: reader.read_u64().await?,
            }),
            3 => Ok(UsersLoadingError::LineTooLong {
                line_number: reader.read_u32().await?,
                byte_at: reader.read_u64().await?,
            }),
            4 => Ok(UsersLoadingError::ExpectedRoleCharGotEOF(
                reader.read_u32().await?,
                reader.read_u32().await?,
            )),
            5 => Ok(UsersLoadingError::InvalidRoleChar(
                reader.read_u32().await?,
                reader.read_u32().await?,
                char::read(reader).await?,
            )),
            6 => Ok(UsersLoadingError::ExpectedColonGotEOF(
                reader.read_u32().await?,
                reader.read_u32().await?,
            )),
            7 => Ok(UsersLoadingError::EmptyUsername(reader.read_u32().await?, reader.read_u32().await?)),
            8 => Ok(UsersLoadingError::UsernameTooLong(
                reader.read_u32().await?,
                reader.read_u32().await?,
            )),
            9 => Ok(UsersLoadingError::EmptyPassword(reader.read_u32().await?, reader.read_u32().await?)),
            10 => Ok(UsersLoadingError::PasswordTooLong(
                reader.read_u32().await?,
                reader.read_u32().await?,
            )),
            11 => Ok(UsersLoadingError::NoUsers),
            _ => Err(io::Error::new(ErrorKind::InvalidData, "Invalid UsersLoadingError type byte")),
        }
    }
}

impl ByteWrite for AuthMethod {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8(*self as u8).await
    }
}

impl ByteRead for AuthMethod {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let value = reader.read_u8().await?;
        AuthMethod::from_u8(value).ok_or(io::Error::new(ErrorKind::InvalidData, "Invalid AuthMethod type byte"))
    }
}

impl ByteWrite for UserRole {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8(*self as u8).await
    }
}

impl ByteRead for UserRole {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match UserRole::from_u8(reader.read_u8().await?) {
            Some(role) => Ok(role),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid UserRole type byte")),
        }
    }
}

impl<T: ByteWrite> ByteWrite for &T {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (*self).write(writer).await
    }
}

impl ByteWrite for SocksRequestAddress {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Self::IPv4(v4) => (4u8, v4).write(writer).await,
            Self::IPv6(v6) => (6u8, v6).write(writer).await,
            Self::Domainname(domainname) => (200u8, SmallWriteString(domainname)).write(writer).await,
        }
    }
}

impl ByteRead for SocksRequestAddress {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match reader.read_u8().await? {
            4 => Ok(SocksRequestAddress::IPv4(Ipv4Addr::read(reader).await?)),
            6 => Ok(SocksRequestAddress::IPv6(Ipv6Addr::read(reader).await?)),
            200 => Ok(SocksRequestAddress::Domainname(SmallReadString::read(reader).await?.0)),
            _ => Err(io::Error::new(ErrorKind::InvalidData, "Invalid SocksRequestAddress type byte")),
        }
    }
}

impl ByteWrite for SocksRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (&self.destination, self.port).write(writer).await
    }
}

impl ByteRead for SocksRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(SocksRequest::new(
            SocksRequestAddress::read(reader).await?,
            reader.read_u16().await?,
        ))
    }
}

impl ByteWrite for LogEventType {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Self::NewSocks5Socket(socket_address) => (0x01u8, socket_address).write(writer).await,
            Self::FailedBindSocks5Socket(socket_address, io_error) => (0x02u8, socket_address, io_error).write(writer).await,
            Self::FailedBindAnySocketAborting => writer.write_u8(0x03u8).await,
            Self::RemovedSocks5Socket(socket_address) => (0x04u8, socket_address).write(writer).await,
            Self::NewSandstormSocket(socket_address) => (0x05u8, socket_address).write(writer).await,
            Self::FailedBindSandstormSocket(socket_address, io_error) => (0x06u8, socket_address, io_error).write(writer).await,
            Self::RemovedSandstormSocket(socket_address) => (0x07u8, socket_address).write(writer).await,
            Self::LoadingUsersFromFile(filename) => (0x08u8, filename).write(writer).await,
            Self::UsersLoadedFromFile(filename, result) => (0x09u8, filename, result).write(writer).await,
            Self::StartingUpWithSingleDefaultUser(userpass) => (0x0Au8, userpass).write(writer).await,
            Self::SavingUsersToFile(filename) => (0x0Bu8, filename).write(writer).await,
            Self::UsersSavedToFile(filename, result) => (0x0Cu8, filename, result).write(writer).await,
            Self::UserRegisteredByArgs(username, role) => (0x0Du8, SmallWriteString(username), role).write(writer).await,
            Self::UserReplacedByArgs(username, role) => (0x0Eu8, SmallWriteString(username), role).write(writer).await,
            Self::NewClientConnectionAccepted(client_id, socket_address) => (0x0Fu8, client_id, socket_address).write(writer).await,
            Self::ClientConnectionAcceptFailed(maybe_socket_address, io_error) => {
                (0x10u8, maybe_socket_address, io_error).write(writer).await
            }
            Self::ClientRequestedUnsupportedVersion(client_id, ver) => (0x11u8, client_id, ver).write(writer).await,
            Self::ClientRequestedUnsupportedCommand(client_id, cmd) => (0x12u8, client_id, cmd).write(writer).await,
            Self::ClientRequestedUnsupportedAtyp(client_id, atyp) => (0x13u8, client_id, atyp).write(writer).await,
            Self::ClientSelectedAuthMethod(client_id, auth_method) => (0x14u8, client_id, auth_method).write(writer).await,
            Self::ClientRequestedUnsupportedUserpassVersion(client_id, ver) => (0x15u8, client_id, ver).write(writer).await,
            Self::ClientAuthenticatedWithUserpass(client_id, username, success) => {
                (0x16u8, client_id, SmallWriteString(username), success).write(writer).await
            }
            Self::ClientSocksRequest(client_id, request) => (0x17u8, client_id, request).write(writer).await,
            Self::ClientDnsLookup(client_id, domainname) => (0x18u8, client_id, SmallWriteString(domainname)).write(writer).await,
            Self::ClientAttemptingConnect(client_id, socket_address) => (0x19u8, client_id, socket_address).write(writer).await,
            Self::ClientConnectionAttemptBindFailed(client_id, io_error) => (0x1Au8, client_id, io_error).write(writer).await,
            Self::ClientConnectionAttemptConnectFailed(client_id, io_error) => (0x1Bu8, client_id, io_error).write(writer).await,
            Self::ClientFailedToConnectToDestination(client_id) => (0x1Cu8, client_id).write(writer).await,
            Self::ClientConnectedToDestination(client_id, socket_address) => (0x1Du8, client_id, socket_address).write(writer).await,
            Self::ClientBytesSent(client_id, count) => (0x1Eu8, client_id, count).write(writer).await,
            Self::ClientBytesReceived(client_id, count) => (0x1Fu8, client_id, count).write(writer).await,
            Self::ClientSourceShutdown(client_id) => (0x20u8, client_id).write(writer).await,
            Self::ClientDestinationShutdown(client_id) => (0x21u8, client_id).write(writer).await,
            Self::ClientConnectionFinished(client_id, sent, received, result) => {
                (0x22u8, client_id, sent, received, result).write(writer).await
            }
            Self::NewSandstormConnectionAccepted(manager_id, socket_address) => (0x23u8, manager_id, socket_address).write(writer).await,
            Self::SandstormConnectionAcceptFailed(maybe_socket_address, io_error) => {
                (0x24u8, maybe_socket_address, io_error).write(writer).await
            }
            Self::SandstormRequestedUnsupportedVersion(manager_id, version) => (0x25u8, manager_id, version).write(writer).await,
            Self::SandstormAuthenticatedAs(manager_id, username, success) => (0x26u8, manager_id, username, success).write(writer).await,
            Self::NewSocksSocketRequestedByManager(manager_id, socket_address) => (0x27u8, manager_id, socket_address).write(writer).await,
            Self::RemoveSocksSocketRequestedByManager(manager_id, socket_address) => {
                (0x28u8, manager_id, socket_address).write(writer).await
            }
            Self::NewSandstormSocketRequestedByManager(manager_id, socket_address) => {
                (0x29u8, manager_id, socket_address).write(writer).await
            }
            Self::RemoveSandstormSocketRequestedByManager(manager_id, socket_address) => {
                (0x2Au8, manager_id, socket_address).write(writer).await
            }
            Self::UserRegisteredByManager(manager_id, username, role) => (0x2Bu8, manager_id, username, role).write(writer).await,
            Self::UserUpdatedByManager(manager_id, username, role, password_changed) => {
                (0x2Cu8, manager_id, username, role, password_changed).write(writer).await
            }
            Self::UserDeletedByManager(manager_id, username, role) => (0x2Du8, manager_id, username, role).write(writer).await,
            Self::AuthMethodToggledByManager(manager_id, auth_method, enabled) => {
                (0x2Eu8, manager_id, auth_method, enabled).write(writer).await
            }
            Self::BufferSizeChangedByManager(manager_id, buffer_size) => (0x2Fu8, manager_id, buffer_size).write(writer).await,
            Self::SandstormRequestedShutdown(manager_id) => (0x30u8, manager_id).write(writer).await,
            Self::SandstormConnectionFinished(manager_id, result) => (0x31u8, manager_id, result).write(writer).await,
            Self::ShutdownSignalReceived => 0x32u8.write(writer).await,
        }
    }
}

impl ByteRead for LogEventType {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let t = reader.read_u8().await?;

        match t {
            0x01 => Ok(Self::NewSocks5Socket(SocketAddr::read(reader).await?)),
            0x02 => Ok(Self::FailedBindSocks5Socket(
                SocketAddr::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x03 => Ok(Self::FailedBindAnySocketAborting),
            0x04 => Ok(Self::RemovedSocks5Socket(SocketAddr::read(reader).await?)),
            0x05 => Ok(Self::NewSandstormSocket(SocketAddr::read(reader).await?)),
            0x06 => Ok(Self::FailedBindSandstormSocket(
                SocketAddr::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x07 => Ok(Self::RemovedSandstormSocket(SocketAddr::read(reader).await?)),
            0x08 => Ok(Self::LoadingUsersFromFile(String::read(reader).await?)),
            0x09 => Ok(Self::UsersLoadedFromFile(
                String::read(reader).await?,
                <Result<u64, UsersLoadingError> as ByteRead>::read(reader).await?,
            )),
            0x0A => Ok(Self::StartingUpWithSingleDefaultUser(String::read(reader).await?)),
            0x0B => Ok(Self::SavingUsersToFile(String::read(reader).await?)),
            0x0C => Ok(Self::UsersSavedToFile(
                String::read(reader).await?,
                <Result<u64, io::Error> as ByteRead>::read(reader).await?,
            )),
            0x0D => Ok(Self::UserRegisteredByArgs(
                SmallReadString::read(reader).await?.0,
                UserRole::read(reader).await?,
            )),
            0x0E => Ok(Self::UserReplacedByArgs(
                SmallReadString::read(reader).await?.0,
                UserRole::read(reader).await?,
            )),
            0x0F => Ok(Self::NewClientConnectionAccepted(
                reader.read_u64().await?,
                SocketAddr::read(reader).await?,
            )),
            0x10 => Ok(Self::ClientConnectionAcceptFailed(
                <Option<SocketAddr> as ByteRead>::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x11 => Ok(Self::ClientRequestedUnsupportedVersion(
                reader.read_u64().await?,
                reader.read_u8().await?,
            )),
            0x12 => Ok(Self::ClientRequestedUnsupportedCommand(
                reader.read_u64().await?,
                reader.read_u8().await?,
            )),
            0x13 => Ok(Self::ClientRequestedUnsupportedAtyp(
                reader.read_u64().await?,
                reader.read_u8().await?,
            )),
            0x14 => Ok(Self::ClientSelectedAuthMethod(
                reader.read_u64().await?,
                AuthMethod::read(reader).await?,
            )),
            0x15 => Ok(Self::ClientRequestedUnsupportedUserpassVersion(
                reader.read_u64().await?,
                reader.read_u8().await?,
            )),
            0x16 => Ok(Self::ClientAuthenticatedWithUserpass(
                reader.read_u64().await?,
                String::read(reader).await?,
                bool::read(reader).await?,
            )),
            0x17 => Ok(Self::ClientSocksRequest(
                reader.read_u64().await?,
                SocksRequest::read(reader).await?,
            )),
            0x18 => Ok(Self::ClientDnsLookup(
                reader.read_u64().await?,
                SmallReadString::read(reader).await?.0,
            )),
            0x19 => Ok(Self::ClientAttemptingConnect(
                reader.read_u64().await?,
                SocketAddr::read(reader).await?,
            )),
            0x1A => Ok(Self::ClientConnectionAttemptBindFailed(
                reader.read_u64().await?,
                io::Error::read(reader).await?,
            )),
            0x1B => Ok(Self::ClientConnectionAttemptConnectFailed(
                reader.read_u64().await?,
                io::Error::read(reader).await?,
            )),
            0x1C => Ok(Self::ClientFailedToConnectToDestination(reader.read_u64().await?)),
            0x1D => Ok(Self::ClientConnectedToDestination(
                reader.read_u64().await?,
                SocketAddr::read(reader).await?,
            )),
            0x1E => Ok(Self::ClientBytesSent(reader.read_u64().await?, reader.read_u64().await?)),
            0x1F => Ok(Self::ClientBytesReceived(reader.read_u64().await?, reader.read_u64().await?)),
            0x20 => Ok(Self::ClientSourceShutdown(reader.read_u64().await?)),
            0x21 => Ok(Self::ClientDestinationShutdown(reader.read_u64().await?)),
            0x22 => Ok(Self::ClientConnectionFinished(
                reader.read_u64().await?,
                reader.read_u64().await?,
                reader.read_u64().await?,
                <Result<(), io::Error> as ByteRead>::read(reader).await?,
            )),
            0x23 => Ok(Self::NewSandstormConnectionAccepted(
                reader.read_u64().await?,
                SocketAddr::read(reader).await?,
            )),
            0x24 => Ok(Self::SandstormConnectionAcceptFailed(
                <Option<SocketAddr> as ByteRead>::read(reader).await?,
                io::Error::read(reader).await?,
            )),
            0x25 => Ok(Self::SandstormRequestedUnsupportedVersion(
                reader.read_u64().await?,
                reader.read_u8().await?,
            )),
            0x26 => Ok(Self::SandstormAuthenticatedAs(
                reader.read_u64().await?,
                String::read(reader).await?,
                bool::read(reader).await?,
            )),
            0x27 => Ok(Self::NewSocksSocketRequestedByManager(
                reader.read_u64().await?,
                SocketAddr::read(reader).await?,
            )),
            0x28 => Ok(Self::RemoveSocksSocketRequestedByManager(
                reader.read_u64().await?,
                SocketAddr::read(reader).await?,
            )),
            0x29 => Ok(Self::NewSandstormSocketRequestedByManager(
                reader.read_u64().await?,
                SocketAddr::read(reader).await?,
            )),
            0x2A => Ok(Self::RemoveSandstormSocketRequestedByManager(
                reader.read_u64().await?,
                SocketAddr::read(reader).await?,
            )),
            0x2B => Ok(Self::UserRegisteredByManager(
                reader.read_u64().await?,
                String::read(reader).await?,
                UserRole::read(reader).await?,
            )),
            0x2C => Ok(Self::UserUpdatedByManager(
                reader.read_u64().await?,
                String::read(reader).await?,
                UserRole::read(reader).await?,
                bool::read(reader).await?,
            )),
            0x2D => Ok(Self::UserDeletedByManager(
                reader.read_u64().await?,
                String::read(reader).await?,
                UserRole::read(reader).await?,
            )),
            0x2E => Ok(Self::AuthMethodToggledByManager(
                reader.read_u64().await?,
                AuthMethod::read(reader).await?,
                bool::read(reader).await?,
            )),
            0x2F => Ok(Self::BufferSizeChangedByManager(reader.read_u64().await?, reader.read_u32().await?)),
            0x30 => Ok(Self::SandstormRequestedShutdown(reader.read_u64().await?)),
            0x31 => Ok(Self::SandstormConnectionFinished(
                reader.read_u64().await?,
                <Result<(), io::Error> as ByteRead>::read(reader).await?,
            )),
            0x32 => Ok(Self::ShutdownSignalReceived),
            _ => Err(io::Error::new(ErrorKind::InvalidData, "Invalid LogEventType type byte")),
        }
    }
}

impl ByteWrite for LogEvent {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_i64(self.timestamp).await?;
        self.data.write(writer).await
    }
}

impl ByteRead for LogEvent {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        Ok(LogEvent::new(reader.read_i64().await?, LogEventType::read(reader).await?))
    }
}

impl ByteWrite for SandstormCommandType {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8(*self as u8).await
    }
}

impl ByteRead for SandstormCommandType {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match SandstormCommandType::from_u8(reader.read_u8().await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid SandstormCommandType type byte")),
        }
    }
}

impl ByteWrite for AddUserResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8(*self as u8).await
    }
}

impl ByteRead for AddUserResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match AddUserResponse::from_u8(reader.read_u8().await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid AddUserResponse type byte")),
        }
    }
}

impl ByteWrite for UpdateUserResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8(*self as u8).await
    }
}

impl ByteRead for UpdateUserResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match UpdateUserResponse::from_u8(reader.read_u8().await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid UpdateUserResponse type byte")),
        }
    }
}

impl ByteWrite for DeleteUserResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8(*self as u8).await
    }
}

impl ByteRead for DeleteUserResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match DeleteUserResponse::from_u8(reader.read_u8().await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid DeleteUserResponse type byte")),
        }
    }
}
