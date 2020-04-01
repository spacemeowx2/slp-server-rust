mod server;
mod frame;
mod peer;
mod peer_manager;
mod stream;

pub use server::*;
pub use frame::*;
pub use peer::*;
pub use peer_manager::*;
pub(super) use stream::*;
use std::net::{SocketAddr, Ipv4Addr};

pub type Packet = Vec<u8>;

#[derive(Debug)]
pub struct SendLANEvent {
    from: SocketAddr,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    packet: Packet,
}

#[derive(Debug)]
pub enum Event {
    Close(SocketAddr),
    SendLAN(SendLANEvent),
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
