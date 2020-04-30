use tokio::io::Result;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, mpsc, broadcast};
use tokio::time::{Duration, Instant};
use tokio::select;
use serde::Serialize;
use juniper::GraphQLObject;
use futures::stream::{StreamExt, BoxStream};
use futures::prelude::*;
use crate::util::{FilterSameExt, create_socket};
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;
use tower::{Service};

pub type Packet = Vec<u8>;
pub struct InPacket {
    pub data: Packet,
    pub addr: SocketAddr,
}
pub struct SendTo {
    pub data: Packet,
    pub addrs: Vec<SocketAddr>,
}

impl SendTo {
    pub fn new(addrs: Vec<SocketAddr>, data: Packet) -> Self {
        Self {
            data,
            addrs,
        }
    }
}

impl InPacket {
    pub fn new(addr: SocketAddr, data: Packet) -> Self {
        Self {
            data,
            addr,
        }
    }
}

pub struct Server<S> {
    maker: S,
    map: HashMap<SocketAddr, Peer>,
}

struct Peer {
    addr: SocketAddr,
    last_access: Instant,
    timeout: Duration,
}
impl Peer {
    fn access(&mut self) {
        self.last_access = Instant::now();
    }
    fn expired(&self) -> bool {
        self.last_access.elapsed() > self.timeout
    }
    fn new(addr: SocketAddr) -> Peer {
        Peer {
            addr,
            last_access: Instant::now(),
            timeout: Duration::from_secs(30),
        }
    }
}

impl<S> Server<S>
where
    S: Service<InPacket, Response=SendTo> + Send + 'static,
{
    pub async fn new(maker: S) -> Result<Self> {
        Ok(Self {
            maker,
            map: HashMap::new(),
        })
    }
    pub async fn serve(&mut self, addr: SocketAddr) -> Result<()> {
        let mut socket = create_socket(&addr).await?;
        let (out_packet_tx, mut out_packet_rx) = mpsc::channel::<SendTo>(10);
        let mut map: HashMap<SocketAddr, Peer> = HashMap::new();
        loop {
            let mut buf = vec![0u8; 65536];
            select! {
                Ok((size, addr)) = socket.recv_from(&mut buf) => {
                    buf.truncate(size);
                    let in_packet = InPacket::new(addr, buf);

                    map.entry(addr).or_insert_with(|| Peer::new(addr)).access();
                },
                Some(SendTo { data, addrs }) = out_packet_rx.recv() => {
                    for addr in addrs {
                        // TODO: is it safe to ignore?
                        let _ = socket.send_to(&data, &addr).await;
                    }
                },
                else => break,
            }
        }

        Ok(())
    }
}
