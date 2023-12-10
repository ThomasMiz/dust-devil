use std::net::{Ipv4Addr, Ipv6Addr};

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::socks5::*;

use super::chunk_reader::read_domainname;

#[derive(Debug)]
pub enum SocksError {
    InvalidVersion(u8),
    CommandNotSupported,
    InvalidATYP,
    IO(io::Error),
}

impl From<io::Error> for SocksError {
    fn from(value: io::Error) -> Self {
        SocksError::IO(value)
    }
}

pub struct SocksHandshake {
    pub methods: Vec<u8>,
}

pub async fn parse_handshake<T: AsyncRead + Unpin>(reader: &mut T) -> Result<SocksHandshake, SocksError> {
    let ver = reader.read_u8().await?;
    if ver != 5 {
        return Err(SocksError::InvalidVersion(ver));
    }

    let nmethods = reader.read_u8().await? as usize;
    let mut methods = vec![0u8; nmethods];
    reader.read_exact(&mut methods).await?;

    Ok(SocksHandshake { methods })
}

pub enum SocksRequestAddress {
    IPv4(Ipv4Addr),
    IPv6(Ipv6Addr),
    Domainname(String),
}

pub struct SocksRequest {
    pub destination: SocksRequestAddress,
    pub port: u16,
}

pub async fn parse_request<T: AsyncRead + Unpin>(reader: &mut T) -> Result<SocksRequest, SocksError> {
    let ver = reader.read_u8().await?;
    if ver != 5 {
        return Err(SocksError::InvalidVersion(ver));
    }

    let cmd = reader.read_u8().await?;
    if cmd != 1 {
        return Err(SocksError::CommandNotSupported);
    }

    let _rsv = reader.read_u8().await?;
    let atyp = reader.read_u8().await?;

    let request = match atyp {
        1 => {
            let mut octets = [0u8; 4];
            reader.read_exact(&mut octets).await?;
            let port = reader.read_u16().await?;

            SocksRequest {
                destination: SocksRequestAddress::IPv4(Ipv4Addr::from(octets)),
                port,
            }
        }
        3 => {
            // Note: The "+ 2" is because a later function will append a ":0" to this string, as the function for
            // DNS resolution for some reason also requires indicating a port. This helps avoid a reallocation.
            let domainname = read_domainname(reader, 2).await?;
            let port = reader.read_u16().await?;

            SocksRequest {
                destination: SocksRequestAddress::Domainname(domainname),
                port,
            }
        }
        4 => {
            let mut octets = [0u8; 16];
            reader.read_exact(&mut octets).await?;
            let port = reader.read_u16().await?;

            SocksRequest {
                destination: SocksRequestAddress::IPv6(Ipv6Addr::from(octets)),
                port,
            }
        }
        _ => {
            return Err(SocksError::InvalidATYP);
        }
    };

    Ok(request)
}
