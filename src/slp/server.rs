use tokio::io::Result;
use tokio::net::{UdpSocket};
use tokio::sync::{Mutex, mpsc, broadcast};
use super::{Event, log_warn, ForwarderFrame, Parser, PeerManager, PeerManagerInfo, Packet, spawn_stream, BoxPlugin, BoxPluginType, Context};
use super::{InPacket, OutPacket, OutAddr};
use serde::Serialize;
use juniper::GraphQLObject;
use futures::stream::{StreamExt, BoxStream};
use futures::prelude::*;
use crate::util::{FilterSameExt, create_socket};
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;
use downcast_rs::Downcast;
use std::any::Any;

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
}

impl UDPServer {
    pub async fn new(addr: &SocketAddr, config: UDPServerConfig) -> Result<Self> {
        let inner = Inner::new();
        let inner2 = inner.clone();
        let (event_send, event_recv) = mpsc::channel::<Event>(100);
        let socket = create_socket(addr).await?;
        let peer_manager = PeerManager::new(config.ignore_idle);
        let pm3 = peer_manager.clone();

        tokio::spawn(async {
            if let Err(err) = Self::recv(inner2, socket, pm3, event_send, event_recv).await {
                log::error!("recv thread exited. reason: {:?}", err);
            }
        });
        let info_sender = spawn_stream(&peer_manager, |pm| async move {
            server_info_from_peer(&pm).await
        });

        Ok(Self {
            peer_manager,
            info_sender,
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
            let in_packet = InPacket::new(addr, buffer);

            let frame = match ForwarderFrame::parse(in_packet.as_ref()) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if let ForwarderFrame::Ping(ping) = &frame {
                Self::send_client(inner.clone(), &mut socket, &addr, ping.build()).await;
                continue
            }
            peer_manager.peer_mut(addr, &event_send, |peer| {
                // ignore packet when channel is full
                let _ = peer.on_packet(in_packet);
            }).await;


            while let Ok(event) = event_recv.try_recv() {
                match event {
                    Event::Close(addr) => {
                        peer_manager.remove(&addr).await;
                    },
                    Event::SendLAN(from, packet) => {
                        log_warn(
                            peer_manager.send_lan(
                                &mut socket,
                                from,
                                packet,
                            ).await,
                            "failed to send lan packet"
                        );
                    },
                }
            }
        }
    }
    async fn send_client(inner: Arc<Mutex<Inner>>, socket: &mut UdpSocket, addr: &SocketAddr, packet: Packet) {
        log_warn(
            socket.send_to(&packet, addr).await,
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
        func(self.inner.lock().await.plugin.get(&typ.name()).unwrap().as_any().downcast_ref::<T>())
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
