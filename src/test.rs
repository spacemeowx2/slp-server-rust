use crate::slp::{UDPServer, UDPServerBuilder};
use smoltcp::phy::ChecksumCapabilities;
use smoltcp::wire::*;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

pub const ADDR: &'static str = "127.0.0.1:0";

pub fn make_packet(src_addr: Ipv4Address, dst_addr: Ipv4Address) -> Vec<u8> {
    let repr = Ipv4Repr {
        src_addr,
        dst_addr,
        protocol: IpProtocol::Udp,
        payload_len: 0,
        hop_limit: 64,
    };
    let mut bytes = vec![0xa5; 1 + repr.buffer_len()];
    let mut packet = Ipv4Packet::new_unchecked(&mut bytes[1..]);
    repr.emit(&mut packet, &ChecksumCapabilities::default());
    bytes[0] = 1;

    bytes
}

pub async fn make_server() -> (UDPServer, SocketAddr) {
    let udp_server = UDPServerBuilder::new()
        .build(&ADDR.parse().unwrap())
        .await
        .unwrap();
    let addr = *udp_server.local_addr();

    (udp_server, addr)
}

pub async fn recv_packet(socket: &mut UdpSocket) -> Vec<u8> {
    let mut buf = vec![0u8; 65536];
    let size = timeout(Duration::from_millis(500), socket.recv(&mut buf))
        .await
        .unwrap()
        .unwrap();
    buf.truncate(size);

    buf
}

pub async fn client_connect(addr: SocketAddr) -> UdpSocket {
    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    let _ = socket.connect(addr).await.unwrap();
    socket
}
