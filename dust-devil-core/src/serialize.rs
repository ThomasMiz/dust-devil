use std::{
    io::{self, ErrorKind},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

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

impl ByteWrite for i64 {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_i64(*self).await
    }
}

impl ByteRead for i64 {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        reader.read_i64().await
    }
}

impl ByteWrite for char {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        let mut buf = [0u8; 4];
        let s = self.encode_utf8(&mut buf);
        writer.write_all(s.as_bytes()).await
    }
}

impl ByteRead for char {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        let mut buf = [0u8; 4];
        let mut byte_count = 0;
        loop {
            reader.read_exact(&mut buf[byte_count..(byte_count + 1)]).await?;
            byte_count += 1;
            if let Ok(s) = std::str::from_utf8(&buf[0..byte_count]) {
                return Ok(s.chars().next().unwrap());
            }

            if byte_count == 4 {
                return Err(io::Error::new(ErrorKind::InvalidData, "char is not valid UTF-8"));
            }
        }
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

        writer.write_u8(kind_id).await?;
        self.to_string().write(writer).await
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

        let message = String::read(reader).await?;

        Ok(io::Error::new(error_kind, message))
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

impl<T: ByteWrite> ByteWrite for &T {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        (*self).write(writer).await
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
