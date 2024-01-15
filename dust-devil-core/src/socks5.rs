use std::{
    io::{self, ErrorKind},
    net::{Ipv4Addr, Ipv6Addr},
};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    serialize::{ByteRead, ByteWrite, SmallReadString, SmallWriteString},
    u8_repr_enum::U8ReprEnum,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocksRequestAddress {
    IPv4(Ipv4Addr),
    IPv6(Ipv6Addr),
    Domainname(String),
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
        match u8::read(reader).await? {
            4 => Ok(SocksRequestAddress::IPv4(Ipv4Addr::read(reader).await?)),
            6 => Ok(SocksRequestAddress::IPv6(Ipv6Addr::read(reader).await?)),
            200 => Ok(SocksRequestAddress::Domainname(SmallReadString::read(reader).await?.0)),
            _ => Err(io::Error::new(ErrorKind::InvalidData, "Invalid SocksRequestAddress type byte")),
        }
    }
}

pub struct SocksRequest {
    pub destination: SocksRequestAddress,
    pub port: u16,
}

impl SocksRequest {
    pub fn new(destination: SocksRequestAddress, port: u16) -> Self {
        Self { destination, port }
    }

    pub fn from_ipv4(ipv4: Ipv4Addr, port: u16) -> Self {
        SocksRequest {
            destination: SocksRequestAddress::IPv4(ipv4),
            port,
        }
    }

    pub fn from_ipv6(ipv6: Ipv6Addr, port: u16) -> Self {
        SocksRequest {
            destination: SocksRequestAddress::IPv6(ipv6),
            port,
        }
    }

    pub fn from_domainname(domainname: String, port: u16) -> Self {
        SocksRequest {
            destination: SocksRequestAddress::Domainname(domainname),
            port,
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
            u16::read(reader).await?,
        ))
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    NoAuth = 0x00,
    // GSSAPI = 0x01,
    UsernameAndPassword = 0x02,
}

impl U8ReprEnum for AuthMethod {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(AuthMethod::NoAuth),
            // 0x01 => Some(AuthMethod::GSSAPI),
            0x02 => Some(AuthMethod::UsernameAndPassword),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteWrite for AuthMethod {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.into_u8().write(writer).await
    }
}

impl ByteRead for AuthMethod {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, io::Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(value) => Ok(value),
            None => Err(io::Error::new(ErrorKind::InvalidData, "Invalid AuthMethod type byte")),
        }
    }
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAuth => write!(f, "noauth"),
            Self::UsernameAndPassword => write!(f, "userpass"),
        }
    }
}
