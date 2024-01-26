use std::{io, net::SocketAddr};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

pub async fn create_socket(addr: &SocketAddr) -> io::Result<UdpSocket> {
    let udp = match addr {
        SocketAddr::V4(_) => Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?,
        SocketAddr::V6(_) => Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?,
    };
    udp.set_nonblocking(true)?;

    // 1MB
    let size = 1 * 1024 * 1024;
    udp.set_recv_buffer_size(size)?;
    udp.set_send_buffer_size(size)?;

    udp.bind(&(*addr).into())?;

    Ok(UdpSocket::from_std(udp.into())?)
}
