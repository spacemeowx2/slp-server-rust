mod server;
mod frame;

pub use server::*;
pub use frame::*;
use std::net::SocketAddr;

#[derive(Debug)]
pub enum Event {
    Close(SocketAddr),
    Unicast(SocketAddr, Vec<u8>),
    Broadcast(SocketAddr, Vec<u8>),
}
