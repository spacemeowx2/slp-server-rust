use super::{ACTION_TIMEOUT, InPacket, Packet, SocketAddr};
use tokio::{net::UdpSocket, sync::mpsc};

pub type SendTo = (Packet, Vec<SocketAddr>);
pub type PacketSender = mpsc::Sender<SendTo>;
pub type PacketReceiver = mpsc::Receiver<InPacket>;
pub type SendError = mpsc::error::SendError<SendTo>;

pub fn packet_stream(socket: UdpSocket) -> (PacketSender, PacketReceiver) {
    let (mut rx, mut tx) = socket.split();
    let (mut in_packet_tx, in_packet_rx) = mpsc::channel::<InPacket>(10);
    let (out_packet_tx, mut out_packet_rx) = mpsc::channel::<SendTo>(10);

    tokio::spawn(async move {
        while let Some((packet, addrs)) = out_packet_rx.recv().await {
            for addr in addrs {
                // TODO: is it safe to ignore?
                let _ = tx.send_to(&packet, &addr).await;
            }
        }
        log::info!("Send task down");
    });
    tokio::spawn(async move {
        loop {
            let mut buf = vec![0u8; 65536];
            if let Ok((size, addr)) = rx.recv_from(&mut buf).await {
                buf.truncate(size);
                let in_packet = InPacket::new(addr, buf);

                // TODO: ignore packet drop now. maybe a counter in the future
                let _ = in_packet_tx.send_timeout(in_packet, ACTION_TIMEOUT).await;
            } else {
                break
            }
        }
        log::info!("Recv task down");
    });

    return (out_packet_tx, in_packet_rx)
}
