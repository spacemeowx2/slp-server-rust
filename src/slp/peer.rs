use tokio::sync::mpsc;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::{Instant, timeout};
use super::{Event, log_err, InPacket, OutPacket, OutAddr};
use super::frame::{ForwarderFrame, Parser};

const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, PartialEq)]
pub enum PeerState {
    Connected(Instant),
    Idle,
}

impl PeerState {
    pub fn is_connected(&self) -> bool {
        match self {
            &PeerState::Connected(_) => true,
            _ => false,
        }
    }
    pub fn is_idle(&self) -> bool {
        match self {
            &PeerState::Idle => true,
            _ => false,
        }
    }
}

struct PeerInner {
    rx: mpsc::Receiver<InPacket>,
    addr: SocketAddr,
    event_send: mpsc::Sender<Event>,
}
pub struct Peer {
    sender: mpsc::Sender<InPacket>,
    pub(super) state: PeerState,
}
impl Peer {
    pub fn new(addr: SocketAddr, event_send: mpsc::Sender<Event>) -> Self {
        let (tx, rx) = mpsc::channel::<InPacket>(10);
        tokio::spawn(async move {
            let mut exit_send = event_send.clone();
            let _ = Self::do_packet(PeerInner {
                rx,
                addr,
                event_send,
            }).await.map_err(|e| log::error!("peer task down {:?}", e));
            log_err(exit_send.send(Event::Close(addr)).await, "peer task send close failed");
        });
        Self {
            sender: tx,
            state: PeerState::Idle,
        }
    }
    pub fn on_packet(&mut self, data: InPacket) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let frame = ForwarderFrame::parse(data.as_ref())?;
        let now = Instant::now();
        let state = match (frame, &self.state) {
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
            self.state = state;
        }

        Ok(self.sender.clone().try_send(data)?)
    }
    async fn do_packet(inner: PeerInner) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let PeerInner { mut rx, addr, mut event_send } = inner;
        loop {
            let packet = match timeout(Duration::from_secs(30), rx.recv()).await {
                Ok(Some(packet)) => packet,
                _ => {
                    log::debug!("Timeout {}", addr);
                    break
                },
            };

            let frame = ForwarderFrame::parse(packet.as_ref())?;

            match frame {
                ForwarderFrame::Keepalive => {},
                ForwarderFrame::Ipv4(ipv4) => {
                    event_send.send(Event::SendLAN(
                        addr,
                        OutPacket::new(ipv4.into(), packet.into())
                    )).await?;
                },
                ForwarderFrame::Ipv4Frag(frag) => {
                    event_send.send(Event::SendLAN(
                        addr,
                        OutPacket::new(frag.into(), packet.into())
                    )).await?;
                },
                _ => (),
            }
        }
        Ok(())
    }
}
