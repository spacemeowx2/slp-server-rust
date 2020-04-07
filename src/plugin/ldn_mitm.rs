use crate::slp::plugin::*;
use crate::slp::{FragParser, OutAddr, ForwarderFrame, Parser};
use smoltcp::wire::{UdpPacket, UdpRepr, Ipv4Packet, Ipv4Repr, IpProtocol};
use smoltcp::phy::ChecksumCapabilities;
use tokio::time::{interval, Duration};
use futures::prelude::*;
use std::net::{SocketAddr, Ipv4Addr};

const LDN_MITM_PORT: u16 = 11452;
const SERVER_ADDR: Ipv4Addr = Ipv4Addr::new(10, 13, 37, 0);
const BROADCAST_ADDR: Ipv4Addr = Ipv4Addr::new(10, 13, 255, 255);

lazy_static! {
    static ref SCAN_PACKET: Vec<u8> = {
        vec![
            0x00, 0x14, 0x45, 0x11,     // magic
            0x00,                       // type: scan
            0,                          // compressed
            0, 0,                       // len
            0, 0,                       // decompressed len
            0, 0,                       // reversed
        ]
    };
    static ref HOST: SocketAddr = SocketAddr::new(Ipv4Addr::new(10, 13, 37, 1).into(), 0);
    static ref PACKET: OutPacket = {
        let mut bytes = make_udp(&SCAN_PACKET);
        bytes.insert(0, 1);

        OutPacket::new(OutAddr::new(
            SERVER_ADDR,
            BROADCAST_ADDR,
        ), bytes)
    };
}

fn make_udp(payload: &[u8]) -> Vec<u8> {
    let checksum = ChecksumCapabilities::default();
    let udp_repr = UdpRepr {
        src_port: LDN_MITM_PORT,
        dst_port: LDN_MITM_PORT,
        payload,
    };
    let ip_repr = Ipv4Repr {
        src_addr: SERVER_ADDR.into(),
        dst_addr: BROADCAST_ADDR.into(),
        protocol: IpProtocol::Udp,
        payload_len: udp_repr.buffer_len(),
        hop_limit: 64,
    };
    let mut bytes = vec![0xa5; ip_repr.buffer_len() + udp_repr.buffer_len()];
    let mut udp_packet = UdpPacket::new_unchecked(&mut bytes[ip_repr.buffer_len()..]);
    udp_repr.emit(&mut udp_packet, &SERVER_ADDR.into(), &BROADCAST_ADDR.into(), &checksum);
    let mut ip_packet = Ipv4Packet::new_unchecked(&mut bytes);
    ip_repr.emit(&mut ip_packet, &checksum);

    bytes
}

pub struct LdnMitm {
    frag_parser: FragParser,
}

impl LdnMitm {
    fn new(peer_manager: PeerManager) -> LdnMitm {
        tokio::spawn(async move {
            interval(Duration::from_secs(5))
                .for_each(move |_| {
                    let pm = peer_manager.clone();
                    async move {
                        let _ = pm.send_lan(*HOST, PACKET.clone()).await;
                    }
                })
                .await;
        });
        LdnMitm {
            frag_parser: FragParser::new(),
        }
    }
}

#[async_trait]
impl Plugin for LdnMitm {
    async fn in_packet(&mut self, packet: &InPacket) {
        let _packet = match ForwarderFrame::parse(packet.as_ref()) {
            Ok(ForwarderFrame::Ipv4(ipv4)) => {
                let src_ip = ipv4.src_ip();
                let dst_ip = ipv4.dst_ip();
                let p = Vec::from(ipv4.data());
                Some((src_ip, dst_ip, p))
            },
            Ok(ForwarderFrame::Ipv4Frag(frag)) => {
                let src_ip = frag.src_ip();
                let dst_ip = frag.dst_ip();
                self.frag_parser.process(frag).map(|p| (src_ip, dst_ip, p))
            },
            _ => None,
        };
        // match _packet {
        //     Some((src_ip, dst_ip, packet)) if dst_ip == SERVER_ADDR => {
        //         println!("-> {:?}: {:?}", src_ip, packet);
        //     },
        //     _ => (),
        // }
    }
    async fn out_packet(&mut self, _packet: &OutPacket) {}
}

pub struct LdnMitmType;
lazy_static! {
    pub static ref LDN_MITM_TYPE: BoxPluginType = Box::new(LdnMitmType);
}

impl PluginType for LdnMitmType {
    fn name(&self) -> String {
        "ldn_mitm".to_string()
    }
    fn new(&self, context: Context) -> Box<dyn Plugin + Send + 'static> {
        Box::new(LdnMitm::new(context.peer_manager.clone()))
    }
}

#[cfg(test)]
mod test {
    use super::make_udp;

    #[test]
    fn test_make_udp() {
        let bytes = make_udp(&[0, 1, 2, 3]);
        assert_eq!(&bytes, &[
            0x45, 0x0, 0x0, 0x20, 0x0, 0x0, 0x40, 0x0, 0x40, 0x11, 0x1, 0xb4,
            10, 13, 37, 0,          // src
            10, 13, 255, 255,       // dst
            0x2c, 0xbc, 0x2c, 0xbc, // src port dst port
            0x0, 0xc, 0x6b, 0x40,
            0x0, 0x1, 0x2, 0x3
        ]);
    }
}
