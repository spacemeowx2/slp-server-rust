use crate::slp::plugin::*;
use crate::slp::{ForwarderFrame, FragParser, Parser};
use smoltcp::wire::{IpProtocol, Ipv4Packet, TcpPacket, UdpPacket};
use std::str::FromStr;

#[derive(Debug)]
pub struct RuleParseError(String);
impl std::fmt::Display for RuleParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse rule: {}", self.0)
    }
}
impl std::error::Error for RuleParseError {}

#[derive(Debug, Clone)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl FromStr for Protocol {
    type Err = RuleParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "tcp" => Ok(Protocol::Tcp),
            "udp" => Ok(Protocol::Udp),
            _ => Err(RuleParseError("invalid protocol".to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rule {
    protocol: Protocol,
    dst_port: u16,
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.protocol {
            Protocol::Tcp => write!(f, "tcp:{}", self.dst_port),
            Protocol::Udp => write!(f, "udp:{}", self.dst_port),
        }
    }
}

impl FromStr for Rule {
    type Err = RuleParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(RuleParseError("format error".to_string()));
        }
        let protocol: Protocol = parts[0].parse()?;
        let dst_port: u16 = parts[1]
            .parse()
            .map_err(|_| RuleParseError("format error".to_string()))?;
        Ok(Rule { protocol, dst_port })
    }
}

pub struct BlockerPlugin {
    frag_parser: FragParser,
    block_rules: Vec<Rule>,
}

impl BlockerPlugin {
    fn new() -> Self {
        BlockerPlugin {
            frag_parser: FragParser::new(),
            block_rules: vec![],
        }
    }
    pub fn set_block_rules(&mut self, block_rules: Vec<Rule>) {
        self.block_rules = block_rules;
    }
}

impl Protocol {
    fn hit<P: AsRef<[u8]>>(&self, packet: &Ipv4Packet<P>) -> bool {
        match self {
            Protocol::Tcp => packet.next_header() == IpProtocol::Tcp,
            Protocol::Udp => packet.next_header() == IpProtocol::Udp,
        }
    }
}

impl Rule {
    fn hit<P: AsRef<[u8]>>(&self, packet: &Ipv4Packet<&P>) -> bool {
        if !self.protocol.hit(packet) {
            return false;
        }

        let dst_port = match self.protocol {
            Protocol::Tcp => match TcpPacket::new_checked(&packet.payload()) {
                Ok(p) => p.dst_port(),
                Err(_e) => return false,
            },
            Protocol::Udp => match UdpPacket::new_checked(&packet.payload()) {
                Ok(p) => p.dst_port(),
                Err(_e) => return false,
            },
        };

        dst_port == self.dst_port
    }
}

#[async_trait]
impl Plugin for BlockerPlugin {
    async fn in_packet(&mut self, packet: &InPacket) -> Result<(), ()> {
        let packet = match ForwarderFrame::parse(packet.as_ref()) {
            Ok(ForwarderFrame::Ipv4(ipv4)) => {
                let src_ip = ipv4.src_ip();
                let dst_ip = ipv4.dst_ip();
                let p = Vec::from(ipv4.data());
                Some((src_ip, dst_ip, p))
            }
            Ok(ForwarderFrame::Ipv4Frag(frag)) => {
                let src_ip = frag.src_ip();
                let dst_ip = frag.dst_ip();
                self.frag_parser.process(frag).map(|p| (src_ip, dst_ip, p))
            }
            _ => None,
        };
        let (_, _, packet) = match packet {
            Some(p) => p,
            None => return Ok(()),
        };
        let packet = match Ipv4Packet::new_checked(&packet) {
            Ok(p) => p,
            _ => return Ok(()),
        };
        for r in &self.block_rules {
            if r.hit(&packet) {
                return Err(());
            }
        }
        Ok(())
    }
    async fn out_packet(&mut self, _packet: &Packet, _addrs: &[SocketAddr]) -> Result<(), ()> {
        Ok(())
    }
}

impl PluginType for BlockerPlugin {
    fn create(_: Context) -> BoxPlugin {
        Box::new(BlockerPlugin::new())
    }
}

#[tokio::test]
async fn get_blocker_from_box() {
    let p: BoxPlugin = Box::new(BlockerPlugin::new());
    let t = p.as_any().downcast_ref::<BlockerPlugin>();
    assert!(t.is_some(), "Blocker should be Some");
}
