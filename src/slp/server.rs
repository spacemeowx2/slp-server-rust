use tokio::io::Result;
use tokio::net::{UdpSocket, udp::RecvHalf};
use tokio::sync::{RwLock, mpsc};
use std::net::{SocketAddr, Ipv4Addr};
use std::sync::Arc;
use super::{Event, SendLANEvent, Peer};
use std::collections::HashMap;

struct InnerServer {
    cache: HashMap<SocketAddr, Peer>,
    map: HashMap<Ipv4Addr, SocketAddr>,
}
impl InnerServer {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            map: HashMap::new(),
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
        let (event_send, mut event_recv) = mpsc::channel::<Event>(1);
        let (recv_half, mut send_half) = UdpSocket::bind(addr).await?.split();

        tokio::spawn(async {
            if let Err(err) = Self::recv(recv_half, inner2, event_send).await {
                log::error!("recv thread exited. reason: {:?}", err);
            }
        });
        tokio::spawn(async move {
            let inner = inner3;
            while let Some(event) = event_recv.recv().await {
                match event {
                    Event::Close(addr) => {
                        inner.write().await.cache.remove(&addr);
                    },
                    Event::SendLAN(SendLANEvent{
                        from,
                        src_ip,
                        dst_ip,
                        packet
                    }) => {
                        let inner = &mut inner.write().await;
                        inner.map.insert(src_ip, from);
                        if let Some(addr) = inner.map.get(&dst_ip) {
                            send_half.send_to(&packet, addr).await.unwrap();
                        } else {
                            for (addr, _) in inner.cache.iter() {
                                if &from == addr {
                                    continue;
                                }
                                send_half.send_to(&packet, &addr).await.unwrap();
                            }
                        }
                    },
                    Event::SendClient(addr, packet) => {
                        send_half.send_to(&packet, &addr).await.unwrap();
                    }
                }
            }
            log::info!("event down");
        });

        Ok(Self {
            inner,
        })
    }
    async fn recv(mut recv: RecvHalf, inner: Arc<RwLock<InnerServer>>, event_send: mpsc::Sender<Event>) -> Result<()> {
        loop {
            let mut buffer = vec![0u8; 65536];
            let (size, addr) = recv.recv_from(&mut buffer).await?;
            buffer.truncate(size);

            let cache = &mut inner.write().await.cache;
            let peer = {
                if cache.get(&addr).is_none() {
                    cache.insert(addr, Peer::new(addr, event_send.clone()));
                }
                cache.get_mut(&addr).unwrap()
            };
            peer.on_packet(buffer).await;
        }
    }
    pub async fn online(&self) -> i32 {
        self.inner.read().await.cache.len() as i32
    }
}
