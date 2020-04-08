use crate::slp::plugin::*;
use crate::slp::{FragParser, ForwarderFrame, Parser};
use super::constants::*;
use tokio::time::{interval, Duration};
use futures::prelude::*;

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
