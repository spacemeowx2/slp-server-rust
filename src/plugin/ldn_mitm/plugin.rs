use super::constants::*;
use super::lan_protocol::{LdnPacket, NetworkInfo};
use crate::slp::frame::{ForwarderFrame, FragParser, Parser};
use crate::slp::plugin::*;
use async_graphql::SimpleObject;
use futures::prelude::*;
use serde::Serialize;
use smoltcp::wire::{IpProtocol, Ipv4Packet, UdpPacket};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use tokio_stream::wrappers::IntervalStream;

/// Node infomation
#[derive(SimpleObject, Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NodeInfo {
    ip: String,
    node_id: i32,
    is_connected: bool,
    player_name: String,
}
/// Room infomation
#[derive(SimpleObject, Clone, Debug, Eq, PartialEq, Serialize)]
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
    room_info: Arc<Mutex<HashMap<Ipv4Addr, RoomInfo>>>,
}

impl LdnMitm {
    fn new(peer_manager: PeerManager) -> LdnMitm {
        let room_info = Arc::new(Mutex::new(HashMap::new()));
        let ri = room_info.clone();
        tokio::spawn(
            IntervalStream::new(interval(Duration::from_secs(5))).for_each(move |_| {
                let pm = peer_manager.clone();
                let ri = ri.clone();
                async move {
                    ri.lock().await.clear();
                    let _ = pm.send_broadcast(slp_scan_packet()).await;
                }
            }),
        );
        LdnMitm {
            frag_parser: FragParser::new(),
            room_info,
        }
    }
}

impl LdnMitm {
    pub fn room_info(&self) -> Arc<Mutex<HashMap<Ipv4Addr, RoomInfo>>> {
        self.room_info.clone()
    }
}

#[async_trait]
impl Plugin for LdnMitm {
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
        match packet {
            Some((src_ip, dst_ip, packet)) if dst_ip == SERVER_ADDR => {
                let mut packet = match Ipv4Packet::new_checked(packet) {
                    Ok(p) => p,
                    _ => return Ok(()),
                };
                if packet.protocol() != IpProtocol::Udp {
                    return Ok(());
                }
                let payload = packet.payload_mut();
                let mut packet = match UdpPacket::new_checked(payload) {
                    Ok(p) => p,
                    _ => return Ok(()),
                };
                let payload = packet.payload_mut();

                let packet = match LdnPacket::new(payload) {
                    Ok(p) => p,
                    _ => return Ok(()),
                };
                if packet.typ() != 1 {
                    return Ok(());
                }
                let info = match NetworkInfo::new(packet.payload()) {
                    Ok(info) => info,
                    _ => return Ok(()),
                };
                let nodes: Vec<_> = info
                    .nodes()
                    .into_iter()
                    .map(|node| NodeInfo {
                        ip: node.ip().to_string(),
                        node_id: node.node_id() as i32,
                        is_connected: node.is_connected(),
                        player_name: node.player_name(),
                    })
                    .collect();
                self.room_info.lock().await.insert(
                    src_ip,
                    RoomInfo {
                        ip: src_ip.to_string(),
                        content_id: hex::encode(info.content_id_bytes()),
                        host_player_name: info.host_player_name(),
                        session_id: hex::encode(info.session_id()),
                        node_count_max: info.node_count_max() as i32,
                        node_count: info.node_count() as i32,
                        nodes,
                        advertise_data_len: info.advertise_data_len() as i32,
                        advertise_data: hex::encode(info.advertise_data()),
                    },
                );
            }
            _ => (),
        };
        Ok(())
    }
    async fn out_packet(&mut self, _packet: &Packet, _addrs: &[SocketAddr]) -> Result<(), ()> {
        Ok(())
    }
}

pub struct LdnMitmType;
pub const LDN_MITM_NAME: &str = "ldn_mitm";
pub static LDN_MITM_TYPE: BoxPluginType<LdnMitm> = Box::new(LdnMitmType);

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
