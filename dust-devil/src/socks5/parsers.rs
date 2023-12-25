use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr},
};

use dust_devil_core::socks5::SocksRequest;
use tokio::io::{AsyncRead, AsyncReadExt};

use super::chunk_reader::read_domainname;

#[derive(Debug)]
pub enum ParseHandshakeError {
    InvalidVersion(u8),
    IO(io::Error),
}

impl From<io::Error> for ParseHandshakeError {
    fn from(value: io::Error) -> Self {
        ParseHandshakeError::IO(value)
    }
}

pub struct SocksHandshake {
    pub methods: Vec<u8>,
}

pub async fn parse_handshake<R>(reader: &mut R) -> Result<SocksHandshake, ParseHandshakeError>
where
    R: AsyncRead + Unpin + ?Sized,
{
    let ver = reader.read_u8().await?;
    if ver != 5 {
        return Err(ParseHandshakeError::InvalidVersion(ver));
    }

    let nmethods = reader.read_u8().await? as usize;
    let mut methods = vec![0u8; nmethods];
    reader.read_exact(&mut methods).await?;

    Ok(SocksHandshake { methods })
}

#[derive(Debug)]
pub enum ParseRequestError {
    InvalidVersion(u8),
    CommandNotSupported(u8),
    InvalidATYP(u8),
    IO(io::Error),
}

impl From<io::Error> for ParseRequestError {
    fn from(value: io::Error) -> Self {
        ParseRequestError::IO(value)
    }
}

pub async fn parse_request<R>(reader: &mut R) -> Result<SocksRequest, ParseRequestError>
where
    R: AsyncRead + Unpin + ?Sized,
{
    let ver = reader.read_u8().await?;
    if ver != 5 {
        return Err(ParseRequestError::InvalidVersion(ver));
    }

    let cmd = reader.read_u8().await?;
    if cmd != 1 {
        return Err(ParseRequestError::CommandNotSupported(cmd));
    }

    let _rsv = reader.read_u8().await?;
    let atyp = reader.read_u8().await?;

    let request = match atyp {
        1 => {
            let mut octets = [0u8; 4];
            reader.read_exact(&mut octets).await?;
            let port = reader.read_u16().await?;

            SocksRequest::from_ipv4(Ipv4Addr::from(octets), port)
        }
        3 => {
            // Note: The "+ 2" is because a later function will append a ":0" to this string, as the function for
            // DNS resolution for some reason also requires indicating a port. This helps avoid a reallocation.
            let domainname = read_domainname(reader, 2).await?;
            let port = reader.read_u16().await?;

            SocksRequest::from_domainname(domainname, port)
        }
        4 => {
            let mut octets = [0u8; 16];
            reader.read_exact(&mut octets).await?;
            let port = reader.read_u16().await?;

            SocksRequest::from_ipv6(Ipv6Addr::from(octets), port)
        }
        other => {
            return Err(ParseRequestError::InvalidATYP(other));
        }
    };

    Ok(request)
}
