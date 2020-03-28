use tokio::io::Result;
use tokio::net::{UdpSocket, udp::RecvHalf};
use tokio::sync::{RwLock, mpsc};
use std::net::{SocketAddr, Ipv4Addr};
use std::sync::Arc;
use super::{Event, SendLANEvent, Peer as InnerPeer, log_err, log_warn, ForwarderFrame, Parser};
use std::collections::HashMap;
use serde::Serialize;
use juniper::GraphQLObject;
use std::time::{Instant, Duration};

const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Infomation about this server
#[derive(Clone, Debug, Eq, PartialEq, Serialize, GraphQLObject)]
pub struct ServerInfo {
    /// The number of online clients
    online: i32,
    /// The number of idle clients(not sending packets for 30s)
    idle: i32,
    /// The version of the server
    version: String,
}

#[derive(Debug, PartialEq)]
enum PeerState {
    Connected(Instant),
    Idle,
}

impl PeerState {
    fn is_connected(&self) -> bool {
        match self {
            &PeerState::Connected(_) => true,
            _ => false,
        }
    }
    fn is_idle(&self) -> bool {
        match self {
            &PeerState::Idle => true,
            _ => false,
        }
    }
}

struct Peer {
    state: PeerState,
    peer: InnerPeer,
}

impl Peer {
    fn new(inner: InnerPeer) -> Self {
        Self {
            state: PeerState::Idle,
            peer: inner,
        }
    }
}

impl From<InnerPeer> for Peer {
    fn from(inner: InnerPeer) -> Self {
        Self::new(inner)
    }
}

struct InnerServer {
    cache: HashMap<SocketAddr, Peer>,
    map: HashMap<Ipv4Addr, SocketAddr>,
    ignore_idle: bool,
}
impl InnerServer {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            map: HashMap::new(),
            ignore_idle: true,
        }
    }

    fn server_info(&self) -> ServerInfo {
        let online = self.cache.len() as i32;
        let idle = self.cache.values().filter(|i| i.state.is_idle()).count() as i32;
        ServerInfo {
            online,
            idle,
            version: std::env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

#[derive(Clone)]
pub struct UDPServer {
    inner: Arc<RwLock<InnerServer>>,
}

impl UDPServer {
    pub async fn new(addr: &str) -> Result<Self> {
        let inner = Arc::new(RwLock::new(InnerServer::new()));
        let inner2 = inner.clone();
        let inner3 = inner.clone();
        let (event_send, mut event_recv) = mpsc::channel::<Event>(10);
        let (recv_half, mut send_half) = UdpSocket::bind(addr).await?.split();

        tokio::spawn(async {
            if let Err(err) = Self::recv(recv_half, inner2, event_send).await {
                log::error!("recv thread exited. reason: {:?}", err);
            }
        });
        tokio::spawn(async move {
            let inner = inner3;
            while let Some(event) = event_recv.recv().await {
                let inner = &mut inner.write().await;
                match event {
                    Event::Close(addr) => {
                        inner.cache.remove(&addr);
                    },
                    Event::SendLAN(SendLANEvent{
                        from,
                        src_ip,
                        dst_ip,
                        packet
                    }) => {
                        inner.map.insert(src_ip, from);
                        if let Some(addr) = inner.map.get(&dst_ip) {
                            log_err(send_half.send_to(&packet, addr).await, "failed to send unary packet");
                        } else {
                            for (addr, _) in inner.cache.iter()
                                .filter(|(addr, i)| !inner.ignore_idle || i.state.is_connected() && &&from != addr)
                            {
                                log_err(
                                    send_half.send_to(&packet, &addr).await,
                                    &format!("failed to send to {} when broadcasting", addr)
                                );
                            }
                        }
                    },
                    Event::SendClient(addr, packet) => {
                        log_err(send_half.send_to(&packet, &addr).await, "failed to sendclient");
                    }
                }
            }
            log::error!("event down");
        });

        Ok(Self {
            inner,
        })
    }
    async fn recv(mut recv: RecvHalf, inner: Arc<RwLock<InnerServer>>, mut event_send: mpsc::Sender<Event>) -> Result<()> {
        loop {
            let mut buffer = vec![0u8; 65536];
            let (size, addr) = recv.recv_from(&mut buffer).await?;
            buffer.truncate(size);

            let frame = match ForwarderFrame::parse(&buffer) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if let ForwarderFrame::Ping(ping) = &frame {
                log_warn(
                    event_send.try_send(Event::SendClient(addr, ping.build())),
                    "failed to send pong"
                );
                continue
            }
            let cache = &mut inner.write().await.cache;
            let peer = {
                if cache.get(&addr).is_none() {
                    cache.insert(
                        addr,
                        InnerPeer::new(addr, event_send.clone()).into()
                    );
                }
                cache.get_mut(&addr).unwrap()
            };
            let now = Instant::now();
            let state = match (frame, &peer.state) {
                (ForwarderFrame::Ipv4(..), _) | (ForwarderFrame::Ipv4Frag(..), _) => {
                    Some(PeerState::Connected(now))
                },
                (_, PeerState::Connected(last_time)) if now.duration_since(*last_time) < IDLE_TIMEOUT => {
                    None
                },
                (_, PeerState::Connected(_)) => {
                    Some(PeerState::Idle)
                },
                _ => {
                    None
                },
            };
            if let Some(state) = state {
                peer.state = state;
            }
            peer.peer.on_packet(buffer);
        }
    }
    pub async fn server_info(&self) -> ServerInfo {
        let inner = self.inner.read().await;

        inner.server_info()
    }
}
