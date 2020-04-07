use crate::slp::plugin::*;
use crate::slp::{FragParser, OutAddr, ForwarderFrame, Parser};
use smoltcp::wire::{UdpPacket, UdpRepr};
use tokio::time::{interval, Duration};
use futures::prelude::*;
use std::net::{SocketAddr, Ipv4Addr};

lazy_static! {
    static ref HOST: SocketAddr = SocketAddr::new(Ipv4Addr::new(10, 13, 37, 1).into(), 0);
    static ref PACKET: OutPacket = OutPacket::new(OutAddr::new(
        Ipv4Addr::new(10, 13, 37, 1),
        Ipv4Addr::new(10, 13, 255, 255),
    ), vec![1]);
}

pub struct LdnMitm {
    frag_parser: FragParser,
}

impl LdnMitm {
    fn new(peer_manager: PeerManager) -> LdnMitm {
        tokio::spawn(async move {
            interval(Duration::from_secs(5))
                .then(move |_| {
                    let pm = peer_manager.clone();
                    async move {
                        let _ = pm.send_lan(*HOST, PACKET.clone()).await;
                    }
                })
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
            Ok(ForwarderFrame::Ipv4Frag(frag)) => {
                self.frag_parser.process(frag)
            },
            _ => None,
        };
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
