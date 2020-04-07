use tokio::io::Result;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, mpsc, broadcast};
use super::{Event, log_warn, ForwarderFrame, Parser, PeerManager, PeerManagerInfo, Packet, spawn_stream, BoxPlugin, BoxPluginType, Context};
use super::{packet_stream, PacketSender, PacketReceiver};
use serde::Serialize;
use juniper::GraphQLObject;
use futures::stream::{StreamExt, BoxStream};
use futures::prelude::*;
use crate::util::{FilterSameExt, create_socket};
use crate::plugin;
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;

type ServerInfoStream = BoxStream<'static, ServerInfo>;

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
            _ => continue
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::Other, "Can't find avaliable port"))
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
        let (packet_tx, packet_rx) = packet_stream(socket);
        let peer_manager = PeerManager::new(packet_tx.clone(), config.ignore_idle);

        Self::spawn_recv(&inner, &packet_tx, packet_rx, &peer_manager, &event_send);
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
        tokio::spawn(async move {
            Self::event_task(&inner, event_recv, &peer_manager).await
        });
    }
    async fn event_task(
        inner: &Arc<Mutex<Inner>>,
        event_recv: mpsc::Receiver<Event>,
        peer_manager: &PeerManager,
    ) {
        event_recv.for_each_concurrent(
            4,
            |event| {
                let inner = inner.clone();
                let peer_manager = peer_manager.clone();
                async move {
                    match event {
                        Event::Close(addr) => {
                            peer_manager.remove(&addr).await;
                        },
                        Event::SendLAN(from, out_packet) => {
                            for (_, p) in &mut inner.lock().await.plugin {
                                p.out_packet(&out_packet).await;
                            }
                            log_warn(
                                peer_manager.send_lan(
                                    from,
                                    out_packet,
                                ).await,
                                "failed to send lan packet"
                            );
                        },
                    }
                }
            }
        ).await
    }
    fn spawn_recv(
        inner: &Arc<Mutex<Inner>>,
        packet_tx: &PacketSender,
        packet_rx: PacketReceiver,
        peer_manager: &PeerManager,
        event_send: &mpsc::Sender<Event>,
    ) {
        let inner = inner.clone();
        let peer_manager = peer_manager.clone();
        let packet_tx = packet_tx.clone();
        let event_send = event_send.clone();
        tokio::spawn(async move {
            Self::recv_task(&inner, &packet_tx, packet_rx, &peer_manager, &event_send).await
        });
    }
    async fn recv_task(
        inner: &Arc<Mutex<Inner>>,
        packet_tx: &PacketSender,
        packet_rx: PacketReceiver,
        peer_manager: &PeerManager,
        event_send: &mpsc::Sender<Event>,
    ) {
        packet_rx.for_each_concurrent(
            10,
            |in_packet| {
                let inner = inner.clone();
                let peer_manager = peer_manager.clone();
                let mut tx = packet_tx.clone();
                let event_send = event_send.clone();
                async move {
                    let addr = *in_packet.addr();
                    for (_, p) in &mut inner.lock().await.plugin {
                        p.in_packet(&in_packet).await;
                    }
                    let frame = match ForwarderFrame::parse(in_packet.as_ref()) {
                        Ok(f) => f,
                        Err(_) => return,
                    };
                    if let ForwarderFrame::Ping(ping) = &frame {
                        Self::send_client(&mut tx, vec![addr], ping.build()).await;
                        return
                    }
                    peer_manager.peer_mut(&addr, &event_send, move |peer| {
                        // ignore packet when channel is full
                        let _ = peer.on_packet(in_packet);
                    }).await;
                }
            }
        ).await
    }
    async fn send_client(socket: &mut PacketSender, addr: Vec<SocketAddr>, packet: Packet) {
        log_warn(
            socket.send((packet, addr)).await,
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
    pub async fn add_plugin(&self, typ: &BoxPluginType) {
        let plugin = typ.new(Context::new(&self.peer_manager));
        self.inner.lock().await.plugin.insert(typ.name(), plugin);
    }
    pub async fn get_plugin<'a, T, F, R>(&'a self, typ: &BoxPluginType<T>, func: F) -> R
    where
        T: 'static,
        F: Fn(Option<&T>) -> R
    {
        let inner = self.inner.lock().await;
        let plugin = inner.plugin.get(&typ.name()).unwrap();
        func(plugin.as_any().downcast_ref::<T>())
    }
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
        plugin::register_plugins(&udp_server).await;
        Ok(udp_server)
    }
}


#[cfg(test)]
mod test {
    use crate::plugin::traffic::TRAFFIC_TYPE;
    use super::UDPServerBuilder;
    use crate::test::{make_server, make_packet, recv_packet, client_connect};
    use tokio::net::UdpSocket;
    use smoltcp::wire::*;

    const ADDR: &'static str = "127.0.0.1:12121";

    #[tokio::test]
    async fn test_get_plugin() {
        let udp_server = UDPServerBuilder::new()
            .find_free_port(true)
            .build(&ADDR.parse().unwrap())
            .await
            .unwrap();
        let traffic = udp_server.get_plugin(&TRAFFIC_TYPE, |traffic| {
            traffic.map(Clone::clone)
        }).await;
        assert!(traffic.is_some(), true);
    }

    #[tokio::test]
    async fn test_server() {
        let (_udp_server, addr) = make_server().await;

        let mut socket1 = client_connect(addr).await;
        let mut socket2 = client_connect(addr).await;

        let packet1 = make_packet(
            Ipv4Address::new(10, 13, 37, 100),
            Ipv4Address::new(10, 13, 37, 101)
        );
        let packet2 = make_packet(
            Ipv4Address::new(10, 13, 37, 101),
            Ipv4Address::new(10, 13, 37, 100)
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
