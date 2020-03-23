use tokio::io::Result;
use tokio::net::UdpSocket;
use tokio::sync::{RwLock, mpsc, oneshot};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use lru::LruCache;
use tokio::time::{Instant, timeout_at};

struct PeerInner {
    rx: mpsc::Receiver<Vec<u8>>,
    addr: SocketAddr,
    close_send: mpsc::Sender<SocketAddr>,
}
struct Peer {
    sender: mpsc::Sender<Vec<u8>>,
}
impl Peer {
    fn new(addr: SocketAddr, close_send: mpsc::Sender<SocketAddr>) -> Self {
        let (tx, rx) = mpsc::channel::<Vec<u8>>(10);
        tokio::spawn(async move {
            Self::on_packet(PeerInner {
                rx,
                addr,
                close_send,
            }).await;
        });
        Self {
            sender: tx,
        }
    }
    async fn send(&self, data: Vec<u8>) {
        self.sender.clone().send(data).await.unwrap()
    }
    async fn on_packet(inner: PeerInner) {
        let PeerInner { mut rx, addr, mut close_send } = inner;
        loop {
            let deadline = Instant::now() + Duration::from_secs(30);
            let packet = match timeout_at(deadline, rx.recv()).await {
                Ok(Some(packet)) => packet,
                _ => {
                    log::info!("Timeout");
                    break
                },
            };
            log::info!("on_packet: {:?} {}", packet, addr);
        }
        close_send.send(addr).await.unwrap();
    }
}

struct InnerServer {
    cache: LruCache<SocketAddr, Peer>,
}
impl InnerServer {
    fn new() -> Self {
        Self {
            cache: LruCache::new(100),
        }
    }
}

#[derive(Clone)]
pub struct UDPServer {
    inner: Arc<RwLock<InnerServer>>,
}

type Packet = (SocketAddr, Vec<u8>);
impl UDPServer {
    pub fn new(addr: String) -> Self {
        let inner = Arc::new(RwLock::new(InnerServer::new()));
        let inner2 = inner.clone();
        let inner3 = inner.clone();
        let (tx, rx) = mpsc::channel::<Packet>(10);
        let (close_send, mut close_recv) = mpsc::channel::<SocketAddr>(1);

        tokio::spawn(async {
            if let Err(err) = Self::recv(tx, addr).await {
                log::error!("recv thread exited. reason: {:?}", err);
            }
        });
        tokio::spawn(async move {
            if let Err(err) = Self::on_packet(rx, inner2, close_send).await {
                log::error!("on_packet thread exited. reason: {:?}", err);
            }
        });
        tokio::spawn(async move {
            let inner = inner3;
            while let Some(addr) = close_recv.recv().await {
                inner.write().await.cache.pop(&addr);
            }
        });

        Self {
            inner,
        }
    }
    async fn on_packet(mut receiver: mpsc::Receiver<Packet>, inner: Arc<RwLock<InnerServer>>, close_send: mpsc::Sender<SocketAddr>) -> Result<()> {
        loop {
            let (addr, buffer) = receiver.recv().await.unwrap();
            let cache = &mut inner.write().await.cache;
            let peer = {
                if cache.peek(&addr).is_none() {
                    cache.put(addr, Peer::new(addr, close_send.clone()));
                }
                cache.get_mut(&addr).unwrap()
            };
            peer.send(buffer).await;
        }
    }
    async fn recv(mut sender: mpsc::Sender<Packet>, addr: String) -> Result<()> {
        let mut socket = UdpSocket::bind(addr).await?;

        loop {
            let mut buffer = vec![0u8; 65536];
            let (size, addr) = socket.recv_from(&mut buffer).await?;
            buffer.truncate(size);

            sender.send((addr, buffer)).await.unwrap();
        }
    }
    pub async fn online(&self) -> i32 {
        self.inner.read().await.cache.len() as i32
    }
}
