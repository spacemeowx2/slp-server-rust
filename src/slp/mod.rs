mod server;
mod frame;
mod peer;
mod peer_manager;
mod stream;
pub mod plugin;
mod packet;

pub use server::*;
pub use frame::*;
pub use peer::*;
pub use peer_manager::*;
pub(super) use stream::*;
pub use plugin::*;
pub use packet::*;
use std::net::SocketAddr;

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
