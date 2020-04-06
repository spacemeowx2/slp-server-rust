use std::net::Ipv4Addr;
use lru::LruCache;
use bytes::Buf;
use super::packet::{Packet, OutAddr};

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
    pub const SRC_IP: Field = 12..16;
    pub const DST_IP: Field = 16..20;
    pub const FRAG_SRC_IP: Field = 0..4;
    pub const FRAG_DST_IP: Field = 4..8;
    pub const FRAG_ID: Field = 8..10;
    pub const FRAG_PART: usize = 10;
    pub const FRAG_TOTAL_PART: usize = 11;
    pub const FRAG_LEN: Field = 12..14;
    pub const FRAG_PMTU: Field = 14..16;
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
        Self: Sized,
    {
        if bytes.len() < Self::MIN_LENGTH || bytes.len() > Self::MAX_LENGTH {
            Err(ParseError::NotParseable)
        } else {
            Self::do_parse(bytes)
        }
    }
}

#[derive(Debug)]
pub enum ForwarderFrame<'a> {
    Keepalive,
    Ipv4(Ipv4<'a>),
    Ping(Ping<'a>),
    Ipv4Frag(Ipv4Frag<'a>),
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
            forwarder_type::IPV4 => ForwarderFrame::Ipv4(Ipv4::parse(&rest)?),
            forwarder_type::PING => ForwarderFrame::Ping(Ping::parse(&rest)?),
            forwarder_type::IPV4_FRAG => ForwarderFrame::Ipv4Frag(Ipv4Frag::parse(&rest)?),
            forwarder_type::AUTH_ME => ForwarderFrame::AuthMe,
            forwarder_type::INFO => ForwarderFrame::Info,
            _ => return Err(ParseError::NotParseable),
        };
        Ok(frame)
    }
}


#[derive(Debug)]
pub struct Ipv4<'a> {
    payload: &'a [u8]
}

impl<'a> Into<OutAddr> for Ipv4<'a> {
    fn into(self) -> OutAddr {
        OutAddr::new(self.src_ip(), self.dst_ip())
    }
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
        octets.copy_from_slice(&self.payload[field::SRC_IP]);
        octets.into()
    }
    pub fn dst_ip(&self) -> Ipv4Addr {
        let mut octets = [0u8; 4];
        octets.copy_from_slice(&self.payload[field::DST_IP]);
        octets.into()
    }
}

#[derive(Debug)]
pub struct Ipv4Frag<'a> {
    payload: &'a [u8]
}

impl<'a> Into<OutAddr> for Ipv4Frag<'a> {
    fn into(self) -> OutAddr {
        OutAddr::new(self.src_ip(), self.dst_ip())
    }
}

impl<'a> Parser<'a> for Ipv4Frag<'a> {
    const MIN_LENGTH: usize = 20;
    fn do_parse(bytes: &'a [u8]) -> Result<Ipv4Frag> {
        Ok(Ipv4Frag { payload: bytes })
    }
}

impl<'a> Ipv4Frag<'a> {
    pub fn src_ip(&self) -> Ipv4Addr {
        let mut octets = [0u8; 4];
        octets.copy_from_slice(&self.payload[field::FRAG_SRC_IP]);
        octets.into()
    }
    pub fn dst_ip(&self) -> Ipv4Addr {
        let mut octets = [0u8; 4];
        octets.copy_from_slice(&self.payload[field::FRAG_DST_IP]);
        octets.into()
    }
    pub fn id(&self) -> u16 {
        let mut buf = &self.payload[field::FRAG_ID];
        buf.get_u16()
    }
    pub fn part(&self) -> u8 {
        self.payload[field::FRAG_PART]
    }
    pub fn total_part(&self) -> u8 {
        self.payload[field::FRAG_TOTAL_PART]
    }
    pub fn len(&self) -> u16 {
        let mut buf = &self.payload[field::FRAG_LEN];
        buf.get_u16()
    }
    pub fn pmtu(&self) -> u16 {
        let mut buf = &self.payload[field::FRAG_PMTU];
        buf.get_u16()
    }
}


#[derive(Debug)]
pub struct Ping<'a> {
    payload: &'a [u8]
}

impl<'a> Parser<'a> for Ping<'a> {
    const MIN_LENGTH: usize = 4;
    const MAX_LENGTH: usize = 4;
    fn do_parse(bytes: &'a [u8]) -> Result<Ping> {
        Ok(Ping { payload: bytes })
    }
}

impl<'a> Ping<'a> {
    pub fn build(&self) -> Vec<u8> {
        let mut out = vec![forwarder_type::PING];
        out.extend_from_slice(&self.payload[0..4]);
        out
    }
}

#[derive(Debug, Clone)]
struct FragItem {
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    id: u16,
    part: u8,
    total_part: u8,
    len: u16,
    pmtu: u16,
    packet: Packet,
}

impl FragItem {
    fn from_frame<'a>(frag: Ipv4Frag<'a>) -> Self {
        FragItem {
            src_ip: frag.src_ip(),
            dst_ip: frag.dst_ip(),
            id: frag.id(),
            part: frag.part(),
            total_part: frag.total_part(),
            len: frag.len(),
            pmtu: frag.pmtu(),
            packet: vec![],
        }
    }
}

pub struct FragParser {
    cache: LruCache<u16, Vec<Option<FragItem>>>,
}

impl FragParser {
    fn process<'a>(&mut self, frame: Ipv4Frag<'a>) -> Option<Packet> {
        let item = FragItem::from_frame(frame);
        let id = item.id;
        let list = if let Some(list) = self.cache.get_mut(&id) {
            list
        } else {
            let mut vec = Vec::new();
            vec.resize(item.total_part as usize, None);
            self.cache.put(id, vec);
            self.cache.get_mut(&id).unwrap()
        };
        let part = item.part as usize;
        list[part] = Some(item);
        if list.iter().all(|i| i.is_some()) {
            None
        } else {
            None
        }
    }
}
