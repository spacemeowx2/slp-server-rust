mod frame;
mod packet;
mod packet_stream;
mod peer;
mod peer_manager;
pub mod plugin;
mod server;
mod stream;

pub use frame::*;
pub use packet::*;
pub use packet_stream::*;
pub use peer::*;
pub use peer_manager::*;
pub use plugin::*;
pub use server::*;
use std::net::SocketAddr;
pub(super) use stream::*;
use tokio::time::Duration;

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
