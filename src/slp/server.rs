use super::{
    frame::{ForwarderFrame, Parser},
    log_warn,
    peer_manager::{PeerManager, PeerManagerInfo},
    plugin::{BoxPlugin, BoxPluginType, Context},
    stream::spawn_stream,
    Event, InPacket, Packet,
};
use crate::util::{create_socket, FilterSameExt};
use async_graphql::SimpleObject;
use futures::prelude::*;
use futures::stream::{BoxStream, StreamExt};
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::Result;
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};

type ServerInfoStream = BoxStream<'static, ServerInfo>;

/// Infomation about this server
#[derive(SimpleObject, Clone, Debug, Eq, PartialEq, Serialize)]
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
    find_free_port: bool,
}

pub struct Inner {
    plugin: HashMap<String, BoxPlugin>,
}

impl Inner {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            plugin: HashMap::new(),
        }))
    }
}

#[derive(Clone)]
pub struct UDPServer {
    peer_manager: PeerManager,
    info_sender: broadcast::Sender<ServerInfo>,
    inner: Arc<Mutex<Inner>>,
    local_addr: SocketAddr,
}

async fn find_port(mut addr: SocketAddr) -> Result<(SocketAddr, UdpSocket)> {
    for port in addr.port()..65535 {
        addr.set_port(port);
        match create_socket(&addr).await {
            Ok(l) => return Ok((addr, l)),
            _ => continue,
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Can't find avaliable port",
    ))
}

impl UDPServer {
    pub async fn new(addr: &SocketAddr, config: UDPServerConfig) -> Result<Self> {
        let inner = Inner::new();
        let (event_send, event_recv) = mpsc::channel::<Event>(100);
        let (local_addr, socket) = if config.find_free_port {
            find_port(*addr).await?
        } else {
            (*addr, create_socket(addr).await?)
        };
        let shared_socket = Arc::new(socket);
        let peer_manager = PeerManager::new(shared_socket.clone(), config.ignore_idle);

        Self::spawn_recv(&inner, shared_socket.clone(), &peer_manager, &event_send);
        Self::spawn_event(&inner, event_recv, &peer_manager);

        let info_sender = spawn_stream(&peer_manager, |pm| async move {
            server_info_from_peer(&pm).await
        });

        Ok(Self {
            peer_manager,
            info_sender,
            inner,
            local_addr,
        })
    }
    fn spawn_event(
        inner: &Arc<Mutex<Inner>>,
        event_recv: mpsc::Receiver<Event>,
        peer_manager: &PeerManager,
    ) {
        let inner = inner.clone();
        let peer_manager = peer_manager.clone();
        tokio::spawn(async move { Self::event_task(&inner, event_recv, &peer_manager).await });
    }
    async fn event_task(
        inner: &Arc<Mutex<Inner>>,
        event_recv: mpsc::Receiver<Event>,
        peer_manager: &PeerManager,
    ) {
        ReceiverStream::new(event_recv)
            .for_each_concurrent(4, |event| {
                let inner = inner.clone();
                let peer_manager = peer_manager.clone();
                async move {
                    match event {
                        Event::Close(addr) => {
                            peer_manager.remove(&addr).await;
                        }
                        Event::SendLAN(from, out_packet) => {
                            let (packet, out_addr) = out_packet.split();
                            let addrs = peer_manager.get_dest_sockaddr(from, out_addr).await;
                            for (_, p) in &mut inner.lock().await.plugin {
                                if p.out_packet(&packet, &addrs).await.is_err() {
                                    return;
                                }
                            }
                            log_warn(
                                peer_manager.send_lan(&packet, addrs).await,
                                "failed to send lan packet",
                            );
                        }
                    }
                }
            })
            .await
    }
    fn spawn_recv(
        inner: &Arc<Mutex<Inner>>,
        udp_socket: Arc<UdpSocket>,
        peer_manager: &PeerManager,
        event_send: &mpsc::Sender<Event>,
    ) {
        let inner = inner.clone();
        let udp_socket = udp_socket.clone();
        let peer_manager = peer_manager.clone();
        let event_send = event_send.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::recv_task(&inner, udp_socket, &peer_manager, &event_send).await {
                log::error!("Recv task down: {:?}", e);
            }
        });
    }
    async fn recv_task(
        inner: &Arc<Mutex<Inner>>,
        udp_socket: Arc<UdpSocket>,
        peer_manager: &PeerManager,
        event_send: &mpsc::Sender<Event>,
    ) -> std::io::Result<()> {
        loop {
            let mut buf = vec![0u8; 65536];
            let (size, addr) = udp_socket.recv_from(&mut buf).await?;
            buf.truncate(size);

            let in_packet = InPacket::new(addr, buf);

            let inner = inner.clone();
            let peer_manager = peer_manager.clone();
            let event_send = event_send.clone();

            let addr = *in_packet.addr();
            for (_, p) in &mut inner.lock().await.plugin {
                if p.in_packet(&in_packet).await.is_err() {
                    continue;
                }
            }
            let frame = match ForwarderFrame::parse(in_packet.as_ref()) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if let ForwarderFrame::Ping(ping) = &frame {
                Self::send_client(&udp_socket, vec![addr], &ping.build()).await;
                continue;
            }
            peer_manager
                .peer_mut(&addr, &event_send, move |peer| {
                    // ignore packet when channel is full
                    let _ = peer.on_packet(in_packet);
                })
                .await;
        }
    }
    async fn send_client(socket: &UdpSocket, addrs: Vec<SocketAddr>, packet: &Packet) {
        for addr in addrs {
            log_warn(
                socket.send_to(packet, addr).await,
                "failed to send client packet",
            );
        }
    }
    pub async fn server_info(&self) -> ServerInfo {
        server_info_from_peer(&self.peer_manager).await
    }
    pub async fn server_info_stream(&self) -> ServerInfoStream {
        let stream = BroadcastStream::new(self.info_sender.subscribe())
            .take_while(|info| future::ready(info.is_ok()))
            .map(|info| info.unwrap());

        stream::once(future::ready(self.server_info().await))
            .chain(stream)
            .filter_same()
            .boxed()
    }
    pub async fn add_plugin(&self, typ: &BoxPluginType) {
        let plugin = typ.new(Context::new(&self.peer_manager));
        self.inner.lock().await.plugin.insert(typ.name(), plugin);
    }
    pub async fn get_plugin<'a, T, F, R>(&'a self, typ: &BoxPluginType<T>, func: F) -> R
    where
        T: 'static,
        F: Fn(Option<&mut T>) -> R,
    {
        let mut inner = self.inner.lock().await;
        let plugin = inner.plugin.get_mut(&typ.name()).unwrap();
        func(plugin.as_any_mut().downcast_mut::<T>())
    }
    #[allow(dead_code)]
    pub fn local_addr(&self) -> &SocketAddr {
        &self.local_addr
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
            find_free_port: false,
        })
    }
    #[allow(dead_code)]
    pub fn find_free_port(mut self, v: bool) -> Self {
        self.0.find_free_port = v;
        self
    }
    pub fn ignore_idle(mut self, v: bool) -> Self {
        self.0.ignore_idle = v;
        self
    }
    pub async fn build(self, addr: &SocketAddr) -> Result<UDPServer> {
        let udp_server = UDPServer::new(addr, self.0).await?;
        Ok(udp_server)
    }
}

#[cfg(test)]
mod test {
    use super::UDPServerBuilder;
    use crate::plugin::{self, traffic::TRAFFIC_TYPE};
    use crate::test::{client_connect, make_packet, make_server, recv_packet};
    use smoltcp::wire::*;

    const ADDR: &'static str = "127.0.0.1:12121";

    #[tokio::test]
    async fn test_get_plugin() {
        let udp_server = UDPServerBuilder::new()
            .find_free_port(true)
            .build(&ADDR.parse().unwrap())
            .await
            .unwrap();
        plugin::register_plugins(&udp_server).await;
        let traffic = udp_server
            .get_plugin(&TRAFFIC_TYPE, |traffic| traffic.map(|t| t.clone()))
            .await;
        assert!(traffic.is_some(), "Traffic should be Some");
    }

    #[tokio::test]
    async fn test_server() {
        let (_udp_server, addr) = make_server().await;

        let mut socket1 = client_connect(addr).await;
        let mut socket2 = client_connect(addr).await;

        let packet1 = make_packet(
            Ipv4Address::new(10, 13, 37, 100),
            Ipv4Address::new(10, 13, 37, 101),
        );
        let packet2 = make_packet(
            Ipv4Address::new(10, 13, 37, 101),
            Ipv4Address::new(10, 13, 37, 100),
        );
        socket1.send(&packet1).await.unwrap();
        socket2.send(&packet2).await.unwrap();
        socket1.send(&packet1).await.unwrap();

        let recv1 = recv_packet(&mut socket1).await;
        let recv2 = recv_packet(&mut socket2).await;

        assert_eq!(recv1, packet2);
        assert_eq!(recv2, packet1);
    }
}
