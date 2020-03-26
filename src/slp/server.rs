use tokio::io::Result;
use tokio::net::{UdpSocket, udp::RecvHalf};
use tokio::sync::{RwLock, mpsc};
use std::net::{SocketAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{Instant, timeout_at};
use super::{Event, SendLANEvent};
use super::frame::{ForwarderFrame, Parser};
use std::collections::HashMap;

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
            let mut exit_send = event_send.clone();
            if Self::do_packet(PeerInner {
                rx,
                addr,
                event_send,
            }).await.is_err() {
                log::warn!("peer task down")
            };
            exit_send.send(Event::Close(addr)).await.unwrap();
        });
        Self {
            sender: tx,
        }
    }
    async fn on_packet(&self, data: Vec<u8>) {
        self.sender.clone().send(data).await.unwrap()
    }
    async fn do_packet(inner: PeerInner) -> std::result::Result<(), Box<dyn std::error::Error>> {
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

            let frame = ForwarderFrame::parse(&packet)?;
            match frame {
                ForwarderFrame::Keepalive => {},
                ForwarderFrame::Ipv4(ipv4) => {
                    event_send.send(Event::SendLAN(SendLANEvent{
                        from: addr,
                        src_ip: ipv4.src_ip(),
                        dst_ip: ipv4.dst_ip(),
                        packet,
                    })).await?
                },
                ForwarderFrame::Ping(ping) => {
                    event_send.send(Event::SendClient(addr, ping.build())).await?
                },
                ForwarderFrame::Ipv4Frag(frag) => {
                    event_send.send(Event::SendLAN(SendLANEvent{
                        from: addr,
                        src_ip: frag.src_ip(),
                        dst_ip: frag.dst_ip(),
                        packet,
                    })).await?
                },
                _ => (),
            }
        }
        Ok(())
    }
}

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
            println!("event down");
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
