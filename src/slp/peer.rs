use tokio::sync::mpsc;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::{Instant, timeout_at};
use super::{Event, SendLANEvent};
use super::frame::{ForwarderFrame, Parser};

struct PeerInner {
    rx: mpsc::Receiver<Vec<u8>>,
    addr: SocketAddr,
    event_send: mpsc::Sender<Event>,
}
pub struct Peer {
    sender: mpsc::Sender<Vec<u8>>,
}
impl Peer {
    pub fn new(addr: SocketAddr, event_send: mpsc::Sender<Event>) -> Self {
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
    pub async fn on_packet(&self, data: Vec<u8>) {
        self.sender.clone().send(data).await.unwrap()
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
