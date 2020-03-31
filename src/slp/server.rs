use tokio::io::Result;
use tokio::net::{UdpSocket};
use tokio::sync::{Mutex, mpsc, broadcast};
use tokio::time::Duration;
use super::{Event, SendLANEvent, log_warn, ForwarderFrame, Parser, PeerManager, PeerManagerInfo, Packet};
use serde::Serialize;
use juniper::GraphQLObject;
use futures::{stream::{StreamExt, BoxStream}};
use futures::prelude::*;
use crate::util::{FilterSameExt, create_socket};
use std::net::SocketAddr;
use std::sync::Arc;
use std::result;

type ServerInfoStream = BoxStream<'static, ServerInfo>;
type TrafficInfoStream = BoxStream<'static, TrafficInfo>;

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

pub struct UDPServerConfig {
    ignore_idle: bool,
}

/// Traffic infomation
#[derive(Clone, Debug, Eq, PartialEq, Serialize, GraphQLObject)]
pub struct TrafficInfo {
    /// upload bytes last second
    upload: i32,
    /// download bytes last second
    download: i32,
}

impl TrafficInfo {
    fn new() -> Self {
        Self {
            upload: 0,
            download: 0,
        }
    }
    fn upload<E>(&mut self, result: result::Result<usize, E>) -> result::Result<usize, E> {
        if let Ok(size) = &result {
            self.upload += *size as i32
        }
        result
    }
    fn download<E>(&mut self, result: result::Result<usize, E>) -> result::Result<usize, E> {
        if let Ok(size) = &result {
            self.download += *size as i32
        }
        result
    }
}

pub struct Inner {
    traffic_info: TrafficInfo,
    last_traffic_info: TrafficInfo,
}

impl Inner {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            traffic_info: TrafficInfo::new(),
            last_traffic_info: TrafficInfo::new(),
        }))
    }
    fn clear_traffic(&mut self) {
        self.last_traffic_info = std::mem::replace(&mut self.traffic_info, TrafficInfo::new());
    }
}

#[derive(Clone)]
pub struct UDPServer {
    peer_manager: PeerManager,
    info_sender: broadcast::Sender<ServerInfo>,
    traffic_sender: broadcast::Sender<TrafficInfo>,
    inner: Arc<Mutex<Inner>>,
}

impl UDPServer {
    pub async fn new(addr: &SocketAddr, config: UDPServerConfig) -> Result<Self> {
        let inner = Inner::new();
        let inner2 = inner.clone();
        let inner5 = inner.clone();
        let (event_send, event_recv) = mpsc::channel::<Event>(100);
        let socket = create_socket(addr).await?;
        let (info_sender, _) = broadcast::channel(1);
        let (traffic_sender, _) = broadcast::channel(1);
        let info_sender2 = info_sender.clone();
        let traffic_sender2 = traffic_sender.clone();
        let peer_manager = PeerManager::new(config.ignore_idle);
        let pm3 = peer_manager.clone();
        let pm4 = peer_manager.clone();

        tokio::spawn(async {
            if let Err(err) = Self::recv(inner2, socket, pm3, event_send, event_recv).await {
                log::error!("recv thread exited. reason: {:?}", err);
            }
        });
        tokio::spawn(tokio::time::interval(Duration::from_secs(1))
            .then(move |_| {
                let pm = pm4.clone();
                async move {
                    server_info_from_peer(&pm).await
                }
            })
            .filter_same()
            .for_each(move |info| {
                // ignore the only error: no active receivers
                let _ = info_sender2.send(info);
                future::ready(())
            })
        );
        tokio::spawn(tokio::time::interval(Duration::from_secs(1))
            .then(move |_| {
                let inner = inner5.clone();
                async move {
                    let mut inner = inner.lock().await;
                    inner.clear_traffic();
                    inner.last_traffic_info.clone()
                }
            })
            .filter_same()
            .for_each(move |info| {
                // ignore the only error: no active receivers
                let _ = traffic_sender2.send(info);
                future::ready(())
            })
        );

        Ok(Self {
            peer_manager,
            info_sender,
            traffic_sender,
            inner,
        })
    }
    async fn recv(
        inner: Arc<Mutex<Inner>>,
        mut socket: UdpSocket,
        peer_manager: PeerManager,
        event_send: mpsc::Sender<Event>,
        mut event_recv: mpsc::Receiver<Event>,
    ) -> Result<()> {
        loop {
            let mut buffer = vec![0u8; 65536];
            let (size, addr) = socket.recv_from(&mut buffer).await?;
            buffer.truncate(size);
            {
                let traffic_info = &mut inner.lock().await.traffic_info;
                let _ = traffic_info.download(result::Result::<_, ()>::Ok(size));
            }

            let frame = match ForwarderFrame::parse(&buffer) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if let ForwarderFrame::Ping(ping) = &frame {
                Self::send_client(inner.clone(), &mut socket, &addr, ping.build()).await;
                continue
            }
            peer_manager.peer_mut(addr, &event_send, |peer| {
                // ignore packet when channel is full
                let _ = peer.on_packet(buffer);
            }).await;


            while let Ok(event) = event_recv.try_recv() {
                match event {
                    Event::Close(addr) => {
                        peer_manager.remove(&addr).await;
                    },
                    Event::SendLAN(SendLANEvent{
                        from,
                        src_ip,
                        dst_ip,
                        packet
                    }) => {
                        let traffic_info = &mut inner.lock().await.traffic_info;
                        log_warn(
                            traffic_info.upload(
                                peer_manager.send_lan(
                                    &mut socket,
                                    packet,
                                    from,
                                    src_ip,
                                    dst_ip,
                                ).await
                            ),
                            "failed to send lan packet"
                        );
                    },
                }
            }
        }
    }
    async fn send_client(inner: Arc<Mutex<Inner>>, socket: &mut UdpSocket, addr: &SocketAddr, packet: Packet) {
        let traffic_info = &mut inner.lock().await.traffic_info;
        log_warn(
            traffic_info.upload(socket.send_to(&packet, addr).await),
            "failed to send client packet",
        );
    }
    pub async fn server_info(&self) -> ServerInfo {
        server_info_from_peer(&self.peer_manager).await
    }
    pub async fn server_info_stream(&self) -> ServerInfoStream {
        let stream = self.info_sender
            .subscribe()
            .take_while(|info| future::ready(info.is_ok()))
            .map(|info| info.unwrap());

        stream::once(future::ready(self.server_info().await))
            .chain(stream)
            .filter_same()
            .boxed()
    }
    pub async fn traffic_info(&self) -> TrafficInfo {
        self.inner.lock().await.last_traffic_info.clone()
    }
    pub async fn traffic_info_stream(&self) -> TrafficInfoStream {
        let stream = self.traffic_sender
            .subscribe()
            .take_while(|info| future::ready(info.is_ok()))
            .map(|info| info.unwrap());

        stream::once(future::ready(self.traffic_info().await))
            .chain(stream)
            .filter_same()
            .boxed()
    }
}

pub async fn server_info_from_peer(peer_manager: &PeerManager) -> ServerInfo {
    let PeerManagerInfo { online, idle } = peer_manager.server_info().await;
    ServerInfo {
        online,
        idle,
        version: std::env!("CARGO_PKG_VERSION").to_owned(),
    }
}

pub struct UDPServerBuilder(UDPServerConfig);

impl UDPServerBuilder {
    pub fn new() -> UDPServerBuilder {
        UDPServerBuilder(UDPServerConfig {
            ignore_idle: false,
        })
    }
    pub fn ignore_idle(mut self, v: bool) -> Self {
        self.0.ignore_idle = v;
        self
    }
    pub async fn build(self, addr: &SocketAddr) -> Result<UDPServer> {
        UDPServer::new(addr, self.0).await
    }
}
