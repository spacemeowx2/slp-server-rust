use tokio::io::Result;
use tokio::net::{UdpSocket, udp::RecvHalf};
use tokio::sync::{RwLock, mpsc};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use lru::LruCache;
use tokio::time::{Instant, timeout_at};

struct PeerInner {
    rx: mpsc::Receiver<Vec<u8>>,
    addr: SocketAddr,
    event_send: mpsc::Sender<Event>,
}
struct Peer {
    sender: mpsc::Sender<Vec<u8>>,
}
impl Peer {
    fn new(addr: SocketAddr, event_send: mpsc::Sender<Event>) -> Self {
        let (tx, rx) = mpsc::channel::<Vec<u8>>(10);
        tokio::spawn(async move {
            Self::do_packet(PeerInner {
                rx,
                addr,
                event_send,
            }).await;
        });
        Self {
            sender: tx,
        }
    }
    async fn on_packet(&self, data: Vec<u8>) {
        self.sender.clone().send(data).await.unwrap()
    }
    async fn do_packet(inner: PeerInner) {
        let PeerInner { mut rx, addr, mut event_send } = inner;
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
        event_send.send(Event::Close(addr)).await.unwrap();
    }
}

#[derive(Debug)]
enum Event {
    Close(SocketAddr),
    Unicast(SocketAddr, Vec<u8>),
    Broadcast(SocketAddr, Vec<u8>),
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
    pub async fn new(addr: String) -> Result<Self> {
        let inner = Arc::new(RwLock::new(InnerServer::new()));
        let inner2 = inner.clone();
        let inner3 = inner.clone();
        let (tx, rx) = mpsc::channel::<Packet>(10);
        let (event_send, mut event_recv) = mpsc::channel::<Event>(1);
        let (recv_half, mut send_half) = UdpSocket::bind(addr).await?.split();

        tokio::spawn(async {
            if let Err(err) = Self::recv(recv_half, tx).await {
                log::error!("recv thread exited. reason: {:?}", err);
            }
        });
        tokio::spawn(async move {
            if let Err(err) = Self::on_packet(rx, inner2, event_send).await {
                log::error!("on_packet thread exited. reason: {:?}", err);
            }
        });
        tokio::spawn(async move {
            let inner = inner3;
            while let Some(event) = event_recv.recv().await {
                match event {
                    Event::Close(addr) => {
                        inner.write().await.cache.pop(&addr);
                    },
                    Event::Unicast(addr, packet) => {
                        send_half.send_to(&packet, &addr).await.unwrap();
                    },
                    Event::Broadcast(exp_addr, packet) => {
                        for (addr, _) in inner.write().await.cache.iter() {
                            if &exp_addr == addr {
                                continue;
                            }
                            send_half.send_to(&packet, &addr).await.unwrap();
                        }
                    }
                }
            }
        });

        Ok(Self {
            inner,
        })
    }
    async fn on_packet(mut receiver: mpsc::Receiver<Packet>, inner: Arc<RwLock<InnerServer>>, event_send: mpsc::Sender<Event>) -> Result<()> {
        loop {
            let (addr, buffer) = receiver.recv().await.unwrap();
            let cache = &mut inner.write().await.cache;
            let peer = {
                if cache.peek(&addr).is_none() {
                    cache.put(addr, Peer::new(addr, event_send.clone()));
                }
                cache.get_mut(&addr).unwrap()
            };
            peer.on_packet(buffer).await;
        }
    }
    async fn recv(mut recv: RecvHalf, mut sender: mpsc::Sender<Packet>) -> Result<()> {
        loop {
            let mut buffer = vec![0u8; 65536];
            let (size, addr) = recv.recv_from(&mut buffer).await?;
            buffer.truncate(size);

            sender.send((addr, buffer)).await.unwrap();
        }
    }
    pub async fn online(&self) -> i32 {
        self.inner.read().await.cache.len() as i32
    }
}
