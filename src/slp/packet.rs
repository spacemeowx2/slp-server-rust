pub use std::net::{SocketAddr, Ipv4Addr};

#[derive(Debug, Clone)]
pub struct OutAddr {
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
}

impl OutAddr {
    pub fn new(
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
    ) -> OutAddr {
        OutAddr {
            src_ip,
            dst_ip,
        }
    }
    pub fn src_ip(&self) -> &Ipv4Addr {
        &self.src_ip
    }
    pub fn dst_ip(&self) -> &Ipv4Addr {
        &self.dst_ip
    }
}


pub type Packet = Vec<u8>;

#[derive(Debug)]
pub struct InPacket(Packet, SocketAddr);

impl InPacket {
    pub fn new(addr: SocketAddr, data: Packet) -> InPacket {
        InPacket(data, addr)
    }
    pub fn addr(&self) -> &SocketAddr {
        &self.1
    }
}

impl Into<Packet> for InPacket {
    fn into(self) -> Packet {
        self.0
    }
}

impl AsRef<[u8]> for InPacket {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct OutPacket(Packet, OutAddr);

impl OutPacket {
    pub fn new(addr: OutAddr, data: Packet) -> OutPacket {
        OutPacket(data, addr)
    }
    pub fn split(self) -> (Packet, OutAddr) {
        (self.0, self.1)
    }
}

impl Into<Packet> for OutPacket {
    fn into(self) -> Packet {
        self.0
    }
}

impl AsRef<[u8]> for OutPacket {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
