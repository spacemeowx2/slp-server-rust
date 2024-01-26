pub(crate) mod frame;
pub(crate) mod packet;
pub(crate) mod packet_stream;
pub(crate) mod peer;
pub(crate) mod peer_manager;
pub mod plugin;
pub(crate) mod server;
pub(crate) mod stream;

use tokio::time::Duration;

pub use frame::{ForwarderFrame, FragParser, Parser};
pub use packet::{InPacket, OutAddr, OutPacket, Packet};
pub use packet_stream::{packet_stream, PacketReceiver, PacketSender, SendError};
pub use peer::{Peer, PeerState};
pub use peer_manager::{PeerManager, PeerManagerInfo};
pub use plugin::{BoxPlugin, BoxPluginType};
pub use server::{ServerInfo, UDPServer, UDPServerBuilder};
pub use std::net::SocketAddr;

pub const ACTION_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub enum Event {
    Close(SocketAddr),
    SendLAN(SocketAddr, OutPacket),
}

pub fn log_err<T, E: std::fmt::Debug>(result: std::result::Result<T, E>, msg: &str) {
    if let Err(e) = result {
        log::error!("{} ({:?})", msg, e);
    }
}

pub fn log_warn<T, E: std::fmt::Debug>(result: std::result::Result<T, E>, msg: &str) {
    if let Err(e) = result {
        log::warn!("{} ({:?})", msg, e)
    }
}
