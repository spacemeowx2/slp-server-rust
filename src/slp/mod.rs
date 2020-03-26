mod server;
mod frame;

pub use server::*;
pub use frame::*;
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
