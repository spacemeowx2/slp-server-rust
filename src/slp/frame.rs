#![allow(dead_code)]
use super::packet::{OutAddr, Packet};
use bytes::Buf;
use lru::LruCache;
use std::{net::Ipv4Addr, num::NonZeroUsize};

mod forwarder_type {
    pub const KEEPALIVE: u8 = 0;
    pub const IPV4: u8 = 1;
    pub const PING: u8 = 2;
    pub const IPV4_FRAG: u8 = 3;
    pub const AUTH_ME: u8 = 4;
    pub const INFO: u8 = 0x10;
}
mod field {
    pub type Field = ::core::ops::Range<usize>;
    pub type FieldFrom = ::core::ops::RangeFrom<usize>;
    pub const SRC_IP: Field = 12..16;
    pub const DST_IP: Field = 16..20;
    pub const FRAG_SRC_IP: Field = 0..4;
    pub const FRAG_DST_IP: Field = 4..8;
    pub const FRAG_ID: Field = 8..10;
    pub const FRAG_PART: usize = 10;
    pub const FRAG_TOTAL_PART: usize = 11;
    pub const FRAG_LEN: Field = 12..14;
    pub const FRAG_PMTU: Field = 14..16;
    pub const FRAG_DATA: FieldFrom = 16..;
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
            forwarder_type::IPV4 => ForwarderFrame::Ipv4(Ipv4::parse(rest)?),
            forwarder_type::PING => ForwarderFrame::Ping(Ping::parse(rest)?),
            forwarder_type::IPV4_FRAG => ForwarderFrame::Ipv4Frag(Ipv4Frag::parse(rest)?),
            forwarder_type::AUTH_ME => ForwarderFrame::AuthMe,
            forwarder_type::INFO => ForwarderFrame::Info,
            _ => return Err(ParseError::NotParseable),
        };
        Ok(frame)
    }
}

#[derive(Debug)]
pub struct Ipv4<'a> {
    payload: &'a [u8],
}

impl<'a> From<Ipv4<'a>> for OutAddr {
    fn from(val: Ipv4<'a>) -> Self {
        OutAddr::new(val.src_ip(), val.dst_ip())
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
    pub fn data(&self) -> &[u8] {
        self.payload
    }
}

#[derive(Debug)]
pub struct Ipv4Frag<'a> {
    payload: &'a [u8],
}

impl<'a> From<Ipv4Frag<'a>> for OutAddr {
    fn from(val: Ipv4Frag<'a>) -> Self {
        OutAddr::new(val.src_ip(), val.dst_ip())
    }
}

impl<'a> Parser<'a> for Ipv4Frag<'a> {
    const MIN_LENGTH: usize = 16;
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
    pub fn data(&self) -> &[u8] {
        &self.payload[field::FRAG_DATA]
    }
}

#[derive(Debug)]
pub struct Ping<'a> {
    payload: &'a [u8],
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
    data: Packet,
}

impl FragItem {
    fn from_frame(frag: Ipv4Frag<'_>) -> Self {
        FragItem {
            src_ip: frag.src_ip(),
            dst_ip: frag.dst_ip(),
            id: frag.id(),
            part: frag.part(),
            total_part: frag.total_part(),
            len: frag.len(),
            pmtu: frag.pmtu(),
            data: Vec::from(frag.data()),
        }
    }
}

type FragParserKey = (Ipv4Addr, u16);
pub struct FragParser {
    cache: LruCache<FragParserKey, Vec<Option<FragItem>>>,
}

impl Default for FragParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FragParser {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(50).unwrap()),
        }
    }
    pub fn process(&mut self, frame: Ipv4Frag<'_>) -> Option<Packet> {
        let src_ip = frame.src_ip();
        let item = FragItem::from_frame(frame);
        let key = (src_ip, item.id);
        let list = if let Some(list) = self.cache.get_mut(&key) {
            list
        } else {
            let mut vec = Vec::new();
            vec.resize(item.total_part as usize, None);
            self.cache.put(key, vec);
            self.cache.get_mut(&key).unwrap()
        };
        let part = item.part as usize;
        match list.get_mut(part) {
            Some(v) => *v = Some(item),
            None => return None,
        };
        if list.iter().all(|i| i.is_some()) {
            let list: Vec<_> = self
                .cache
                .pop(&key)
                .unwrap()
                .into_iter()
                .map(Option::unwrap)
                .collect();
            let size: usize = list.iter().fold(0, |acc, item| acc + item.len as usize);
            let mut packet = vec![0u8; size];
            for i in list {
                let start = i.pmtu as usize * i.part as usize;
                let end = start + i.len as usize;
                match packet.get_mut(start..end) {
                    Some(s) => s.copy_from_slice(&i.data),
                    None => return None,
                }
            }
            Some(packet)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::{FragParser, Ipv4Frag, Parser};

    #[tokio::test]
    async fn frag_parser() {
        let mut parser = FragParser::new();
        let frag1 = Ipv4Frag::parse(&[
            10, 13, 37, 100, // src_ip
            10, 13, 37, 101, // dst_ip
            0, 1, // id
            0, // part
            2, // total_part
            0, 3, // len
            0, 3, // pmtu
            0, 1, 2, // data
        ])
        .unwrap();
        let frag2 = Ipv4Frag::parse(&[
            10, 13, 37, 101, // src_ip
            10, 13, 37, 100, // dst_ip
            0, 1, // id
            1, // part
            2, // total_part
            0, 2, // len
            0, 3, // pmtu
            3, 4, // data
        ])
        .unwrap();
        let frag3 = Ipv4Frag::parse(&[
            10, 13, 37, 101, // src_ip
            10, 13, 37, 100, // dst_ip
            0, 1, // id
            0, // part
            2, // total_part
            0, 3, // len
            0, 3, // pmtu
            0, 1, 2, // data
        ])
        .unwrap();
        let frag4 = Ipv4Frag::parse(&[
            10, 13, 37, 100, // src_ip
            10, 13, 37, 101, // dst_ip
            0, 1, // id
            1, // part
            2, // total_part
            0, 2, // len
            0, 3, // pmtu
            3, 4, // data
        ])
        .unwrap();
        assert_eq!(parser.process(frag1), None);
        assert_eq!(parser.process(frag2), None);
        assert_eq!(parser.process(frag3).unwrap(), vec![0, 1, 2, 3, 4]);
        assert_eq!(parser.process(frag4).unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn frag_parser_fix_panic() {
        let mut parser = FragParser::new();
        let frag1 = Ipv4Frag::parse(&[
            10, 13, 37, 100, // src_ip
            10, 13, 37, 101, // dst_ip
            0, 1, // id
            0, // part
            2, // total_part
            0, 3, // len
            0, 3, // pmtu
            0, 1, 2, // data
        ])
        .unwrap();
        let frag2 = Ipv4Frag::parse(&[
            10, 13, 37, 100, // src_ip
            10, 13, 37, 101, // dst_ip
            0, 1, // id
            1, // part
            2, // total_part
            0, 2, // len
            0, 4, // * wrong pmtu
            3, 4, // data
        ])
        .unwrap();
        let frag3 = Ipv4Frag::parse(&[
            10, 13, 37, 100, // src_ip
            10, 13, 37, 101, // dst_ip
            0, 1,  // id
            10, // * wrong part
            2,  // total_part
            0, 2, // len
            0, 3, // pmtu
            3, 4, // data
        ])
        .unwrap();
        assert_eq!(parser.process(frag1), None);
        assert_eq!(parser.process(frag2), None);
        assert_eq!(parser.process(frag3), None);
    }
}
