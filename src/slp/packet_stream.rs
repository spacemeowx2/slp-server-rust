use super::{InPacket, Packet, SocketAddr, ACTION_TIMEOUT};
use tokio::select;
use tokio::{net::UdpSocket, sync::mpsc};

pub type SendTo = (Packet, Vec<SocketAddr>);
pub type PacketSender = mpsc::Sender<SendTo>;
pub type PacketReceiver = mpsc::Receiver<InPacket>;
pub type SendError = mpsc::error::SendError<SendTo>;

pub fn packet_stream(mut socket: UdpSocket) -> (PacketSender, PacketReceiver) {
    // let (mut rx, tx) = socket.split();
    let (mut in_packet_tx, in_packet_rx) = mpsc::channel::<InPacket>(10);
    let (out_packet_tx, mut out_packet_rx) = mpsc::channel::<SendTo>(10);
    tokio::spawn(async move {
        loop {
            let mut buf = vec![0u8; 65536];
            select! {
                Ok((size, addr)) = socket.recv_from(&mut buf) => {
                    buf.truncate(size);
                    let in_packet = InPacket::new(addr, buf);

                    // TODO: ignore packet drop now. maybe a counter in the future
                    let _ = in_packet_tx.send_timeout(in_packet, ACTION_TIMEOUT).await;
                },
                Some((packet, addrs)) = out_packet_rx.recv() => {
                    for addr in addrs {
                        // TODO: is it safe to ignore?
                        let _ = socket.send_to(&packet, &addr).await;
                    }
                },
                else => break,
            }
        }
        log::error!("Recv task down");
    });
    return (out_packet_tx, in_packet_rx);
}
