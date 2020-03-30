use tokio::io::Result;
use tokio::net::{UdpSocket, udp::RecvHalf};
use tokio::sync::mpsc;
use super::{Event, SendLANEvent, log_warn, ForwarderFrame, Parser, PeerManager, PeerManagerInfo};
use serde::Serialize;
use juniper::GraphQLObject;

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

#[derive(Clone)]
pub struct UDPServer {
    peer_manager: PeerManager,
}

impl UDPServer {
    pub async fn new(addr: &str, config: UDPServerConfig) -> Result<Self> {
        let (event_send, mut event_recv) = mpsc::channel::<Event>(100);
        let (recv_half, mut send_half) = UdpSocket::bind(addr).await?.split();
        let peer_manager = PeerManager::new(config.ignore_idle);
        let pm2 = peer_manager.clone();
        let pm3 = peer_manager.clone();

        tokio::spawn(async {
            if let Err(err) = Self::recv(recv_half, pm3, event_send).await {
                log::error!("recv thread exited. reason: {:?}", err);
            }
        });
        tokio::spawn(async move {
            while let Some(event) = event_recv.recv().await {
                match event {
                    Event::Close(addr) => {
                        pm2.remove(&addr).await;
                    },
                    Event::SendLAN(SendLANEvent{
                        from,
                        src_ip,
                        dst_ip,
                        packet
                    }) => {
                        log_warn(
                            pm2.send_lan(
                                &mut send_half,
                                packet,
                                from,
                                src_ip,
                                dst_ip,
                            ).await,
                            "failed to send lan packet"
                        );
                    },
                    Event::SendClient(addr, packet) => {
                        log_warn(
                            send_half.send_to(&packet, &addr).await,
                            "failed to send client packet",
                        );
                    }
                }
            }
            log::error!("event down");
        });

        Ok(Self {
            peer_manager,
        })
    }
    async fn recv(mut recv: RecvHalf, peer_manager: PeerManager, mut event_send: mpsc::Sender<Event>) -> Result<()> {
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
            peer_manager.peer_mut(addr, &event_send, |peer| {
                log_warn(
                    peer.on_packet(buffer),
                    "peer failed to process packet"
                );
            }).await;
        }
    }
    pub async fn server_info(&self) -> ServerInfo {
        let PeerManagerInfo { online, idle } = self.peer_manager.server_info().await;
        ServerInfo {
            online,
            idle,
            version: std::env!("CARGO_PKG_VERSION").to_owned(),
        }
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
    pub async fn build(self, addr: &str) -> Result<UDPServer> {
        UDPServer::new(addr, self.0).await
    }
}
