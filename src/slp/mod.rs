mod server;
mod frame;
mod peer;

pub use server::*;
pub use frame::*;
pub use peer::*;
use std::net::{SocketAddr, Ipv4Addr};

#[derive(Debug)]
pub struct SendLANEvent {
    from: SocketAddr,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    packet: Vec<u8>,
}

#[derive(Debug)]
pub enum Event {
    Close(SocketAddr),
    SendLAN(SendLANEvent),
    SendClient(SocketAddr, Vec<u8>),
}

pub fn log_err<T, E>(result: std::result::Result<T, E>, msg: &str) {
    if result.is_err() {
        log::error!("{}", msg)
    }
}

pub fn log_warn<T, E>(result: std::result::Result<T, E>, msg: &str) {
    if result.is_err() {
        log::warn!("{}", msg)
    }
}
