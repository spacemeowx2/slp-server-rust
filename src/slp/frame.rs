use std::net::Ipv4Addr;

mod forwarder_type {
    pub const KEEPALIVE: u8   = 0;
    pub const IPV4: u8        = 1;
    pub const PING: u8        = 2;
    pub const IPV4_FRAG: u8    = 3;
    pub const AUTH_ME: u8      = 4;
    pub const INFO: u8        = 0x10;
}
mod field {
    pub type Field = ::core::ops::Range<usize>;
    pub type Rest  = ::core::ops::RangeFrom<usize>;
    pub const SRC_DEST: Field = 12..16;
}

#[derive(Debug, Clone, Copy)]
pub enum ParseError {
    NotParseable,
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "the data is not parseable")
    }
}
impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

pub type Result<T> = std::result::Result<T, ParseError>;

pub trait Parser<'a> {
    const MIN_LENGTH: usize;
    const MAX_LENGTH: usize = 2048;

    fn do_parse(bytes: &'a [u8]) -> Result<Self>
    where
        Self: Sized;

    fn parse(bytes: &'a [u8]) -> Result<Self>
    where
        Self: Sized
    {
        if bytes.len() < Self::MIN_LENGTH || bytes.len() >= Self::MAX_LENGTH {
            Err(ParseError::NotParseable)
        } else {
            Self::do_parse(bytes)
        }
    }
}

pub enum ForwarderFrame<'a> {
    Keepalive,
    Ipv4(Ipv4<'a>),
    Ping(Ping<'a>),
    Ipv4Frag,
    AuthMe,
    Info,
}

impl<'a> Parser<'a> for ForwarderFrame<'a> {
    const MIN_LENGTH: usize = 1;
    fn do_parse(bytes: &'a [u8]) -> Result<ForwarderFrame> {
        let typ = bytes[0];
        let rest = &bytes[1..];
        let frame = match typ {
            forwarder_type::KEEPALIVE => ForwarderFrame::Keepalive,
            forwarder_type::IPV4 => ForwarderFrame::Ipv4(Ipv4::parse(rest)?),
            forwarder_type::PING => ForwarderFrame::Ping(Ping::parse(rest)?),
            forwarder_type::IPV4_FRAG => ForwarderFrame::Ipv4Frag,
            forwarder_type::AUTH_ME => ForwarderFrame::AuthMe,
            forwarder_type::INFO => ForwarderFrame::Info,
            _ => return Err(ParseError::NotParseable),
        };
        Ok(frame)
    }
}


pub struct Ipv4<'a> {
    payload: &'a [u8]
}

impl<'a> Parser<'a> for Ipv4<'a> {
    const MIN_LENGTH: usize = 20;
    fn do_parse(bytes: &'a [u8]) -> Result<Ipv4> {
        Ok(Ipv4 { payload: bytes })
    }
}

impl<'a> Ipv4<'a> {
    pub fn src_ip(&self) -> Ipv4Addr {
        let mut octets = [0u8; 4];
        octets.copy_from_slice(&self.payload[0..4]);
        octets.into()
    }
}


pub struct Ping<'a> {
    payload: &'a [u8]
}

impl<'a> Parser<'a> for Ping<'a> {
    const MIN_LENGTH: usize = 4;
    const MAX_LENGTH: usize = 4;
    fn do_parse(bytes: &'a [u8]) -> Result<Ping> {
        if bytes.len() < 4 {
            Err(ParseError::NotParseable)
        } else {
            Ok(Ping { payload: bytes })
        }
    }
}
