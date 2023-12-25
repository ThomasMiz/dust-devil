use std::net::{Ipv4Addr, Ipv6Addr};

pub enum SocksRequestAddress {
    IPv4(Ipv4Addr),
    IPv6(Ipv6Addr),
    Domainname(String),
}

pub struct SocksRequest {
    pub destination: SocksRequestAddress,
    pub port: u16,
}

impl SocksRequest {
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

#[repr(u8)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AuthMethod {
    NoAuth = 0x00,
    // GSSAPI = 0x01,
    UsernameAndPassword = 0x02,
    NoAcceptableMethod = 0xFF,
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAuth => write!(f, "noauth"),
            Self::UsernameAndPassword => write!(f, "userpass"),
            Self::NoAcceptableMethod => write!(f, "unacceptable"),
        }
    }
}
