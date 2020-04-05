use super::{ACTION_TIMEOUT, InPacket, Packet, SocketAddr};
use tokio::{net::UdpSocket, sync::mpsc};
use tokio::select;

pub type SendTo = (Packet, SocketAddr);
pub type PacketSender = mpsc::Sender<SendTo>;
pub type PacketReceiver = mpsc::Receiver<InPacket>;
pub type SendError = mpsc::error::SendError<SendTo>;

pub fn packet_stream(mut socket: UdpSocket) -> (PacketSender, PacketReceiver) {
    // let (mut rx, tx) = socket.split();
    let (mut in_packet_tx, in_packet_rx) = mpsc::channel::<InPacket>(10);
    let (out_packet_tx, mut out_packet_rx) = mpsc::channel::<SendTo>(10);
    tokio::spawn(async move {
        let timeout = ACTION_TIMEOUT;
        loop {
            let mut buf = vec![0u8; 65536];
            select! {
                Ok((size, addr)) = socket.recv_from(&mut buf) => {
                    buf.truncate(size);
                    let in_packet = InPacket::new(addr, buf);

                    match in_packet_tx.send_timeout(in_packet, timeout).await {
                        Ok(_) => (),
                        Err(e) => {
                            log::error!("send_timeout err {:?}", e);
                        }
                    }
                },
                Some((packet, addr)) = out_packet_rx.recv() => {
                    // TODO: is it safe to ignore?
                    let _ = socket.send_to(&packet, &addr).await;
                },
                else => break,
            }
        }
        log::error!("Recv task down");
    });
    return (out_packet_tx, in_packet_rx)
}
