use crate::slp::plugin::*;
use crate::slp::{FragParser, ForwarderFrame, Parser};
use super::constants::*;
use super::lan_protocol::{LdnPacket, NetworkInfo};
use tokio::time::{interval, Duration};
use futures::prelude::*;
use smoltcp::wire::{Ipv4Packet, UdpPacket, IpProtocol};
use std::collections::HashMap;
use serde::Serialize;
use juniper::GraphQLObject;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Node infomation
#[derive(Clone, Debug, Eq, PartialEq, Serialize, GraphQLObject)]
pub struct NodeInfo {
    ip: String,
    node_id: i32,
    is_connected: bool,
    player_name: String,
}
/// Room infomation
#[derive(Clone, Debug, Eq, PartialEq, Serialize, GraphQLObject)]
pub struct RoomInfo {
    /// the ip of room
    ip: String,
    /// the content id of the game
    content_id: String,
    /// host player name
    host_player_name: String,
    /// session id
    session_id: String,
    /// node count max
    node_count_max: i32,
    /// node count
    node_count: i32,
    /// nodes
    nodes: Vec<NodeInfo>,
    /// advertise data length
    advertise_data_len: i32,
    /// advertise data in hex
    advertise_data: String,
}

pub struct LdnMitm {
    frag_parser: FragParser,
    room_info: Arc<Mutex<HashMap<u64, RoomInfo>>>,
}

impl LdnMitm {
    fn new(peer_manager: PeerManager) -> LdnMitm {
        let room_info = Arc::new(Mutex::new(HashMap::new()));
        let ri = room_info.clone();
        tokio::spawn(
            interval(Duration::from_secs(5))
            .for_each(move |_| {
                let pm = peer_manager.clone();
                let ri = ri.clone();
                async move {
                    ri.lock().await.clear();
                    let _ = pm.send_broadcast(PACKET.clone()).await;
                }
            })
        );
        LdnMitm {
            frag_parser: FragParser::new(),
            room_info,
        }
    }
}

impl LdnMitm {
    pub fn room_info(&self) -> Arc<Mutex<HashMap<u64, RoomInfo>>> {
        self.room_info.clone()
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
        match _packet {
            Some((src_ip, dst_ip, packet)) if dst_ip == SERVER_ADDR => {
                let mut packet = match Ipv4Packet::new_checked(packet) {
                    Ok(p) => p,
                    _ => return,
                };
                if packet.protocol() != IpProtocol::Udp {
                    return
                }
                let payload = packet.payload_mut();
                let mut packet = match UdpPacket::new_checked(payload) {
                    Ok(p) => p,
                    _ => return,
                };
                let payload = packet.payload_mut();

                let packet = match LdnPacket::new(payload) {
                    Ok(p) => p,
                    _ => return,
                };
                if packet.typ() != 1 {
                    return
                }
                let info = match NetworkInfo::new(packet.payload()) {
                    Ok(info) => info,
                    _ => return,
                };
                let nodes: Vec<_> = info.nodes()
                    .into_iter()
                    .map(|node| NodeInfo {
                        ip: node.ip().to_string(),
                        node_id: node.node_id() as i32,
                        is_connected: node.is_connected(),
                        player_name: node.player_name(),
                    })
                    .collect();
                self.room_info.lock().await.insert(info.content_id(), RoomInfo {
                    ip: src_ip.to_string(),
                    content_id: hex::encode(info.content_id_bytes()),
                    host_player_name: info.host_player_name(),
                    session_id: hex::encode(info.session_id()),
                    node_count_max: info.node_count_max() as i32,
                    node_count: info.node_count() as i32,
                    nodes,
                    advertise_data_len: info.advertise_data_len() as i32,
                    advertise_data: hex::encode(info.advertise_data()),
                });
            },
            _ => (),
        }
    }
    async fn out_packet(&mut self, _packet: &Packet, _addrs: &[SocketAddr]) {}
}

pub struct LdnMitmType;
pub const LDN_MITM_NAME: &str = "ldn_mitm";
lazy_static! {
    pub static ref LDN_MITM_TYPE: BoxPluginType<LdnMitm> = Box::new(LdnMitmType);
}

impl PluginType for LdnMitmType {
    fn name(&self) -> String {
        "ldn_mitm".to_string()
    }
    fn new(&self, context: Context) -> Box<dyn Plugin + Send + 'static> {
        Box::new(LdnMitm::new(context.peer_manager.clone()))
    }
}

impl PluginType<LdnMitm> for LdnMitmType {
    fn name(&self) -> String {
        LDN_MITM_NAME.to_string()
    }
    fn new(&self, context: Context) -> BoxPlugin {
        Box::new(LdnMitm::new(context.peer_manager.clone()))
    }
}
