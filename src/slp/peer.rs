use tokio::sync::mpsc;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::{Instant, timeout_at};
use super::{Event, SendLANEvent, log_err, Packet};
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
    rx: mpsc::Receiver<Packet>,
    addr: SocketAddr,
    event_send: mpsc::Sender<Event>,
}
pub struct Peer {
    sender: mpsc::Sender<Packet>,
    pub(super) state: PeerState,
}
impl Peer {
    pub fn new(addr: SocketAddr, event_send: mpsc::Sender<Event>) -> Self {
        let (tx, rx) = mpsc::channel::<Packet>(10);
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
    pub fn on_packet(&mut self, data: Packet) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let frame = ForwarderFrame::parse(&data)?;
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
            let deadline = Instant::now() + Duration::from_secs(30);
            let packet = match timeout_at(deadline, rx.recv()).await {
                Ok(Some(packet)) => packet,
                _ => {
                    log::debug!("Timeout {}", addr);
                    break
                },
            };

            let frame = ForwarderFrame::parse(&packet)?;

            match frame {
                ForwarderFrame::Keepalive => {},
                ForwarderFrame::Ipv4(ipv4) => {
                    let _ = event_send.try_send(Event::SendLAN(SendLANEvent{
                        from: addr,
                        src_ip: ipv4.src_ip(),
                        dst_ip: ipv4.dst_ip(),
                        packet,
                    }));
                },
                ForwarderFrame::Ipv4Frag(frag) => {
                    let _ = event_send.try_send(Event::SendLAN(SendLANEvent{
                        from: addr,
                        src_ip: frag.src_ip(),
                        dst_ip: frag.dst_ip(),
                        packet,
                    }));
                },
                _ => (),
            }
        }
        Ok(())
    }
}
