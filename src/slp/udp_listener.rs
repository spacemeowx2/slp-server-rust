use super::{InPacket, Packet, SocketAddr};
use tokio::{net::UdpSocket, sync::{mpsc, oneshot}};
use tokio::select;
use futures::{ready, stream::Stream};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::collections::HashMap;
use tokio::time::Duration;

// type SendTo = (Packet, SocketAddr, oneshot::Sender<tokio::io::Result<usize>>);
type SendTo = (Packet, SocketAddr);
pub type PacketSender = mpsc::Sender<SendTo>;
pub type PacketReceiver = mpsc::Receiver<InPacket>;
pub type SendError = mpsc::error::SendError<SendTo>;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

type UdpItem = Vec<u8>;

pub struct UdpListener {
    socket_rx: mpsc::Receiver<UdpStream>,
}

impl UdpListener {
    pub fn new(socket: UdpSocket) -> UdpListener {
        Self::with_timeout(socket, DEFAULT_TIMEOUT)
    }
    pub fn with_timeout(socket: UdpSocket, timeout: Duration) -> UdpListener {
        let (socket_tx, socket_rx) = mpsc::channel(1);
        tokio::spawn(UdpListener::run(socket, socket_tx));
        UdpListener {
            socket_rx,
        }
    }
    pub async fn next(&mut self) -> Option<UdpStream> {
        self.socket_rx.recv().await
    }
    async fn run(mut socket: UdpSocket, mut socket_tx: mpsc::Sender<UdpStream>) {
        let (mut in_packet_tx, in_packet_rx) = mpsc::channel::<InPacket>(10);
        let (out_packet_tx, mut out_packet_rx) = mpsc::channel::<SendTo>(10);
        let mut map: HashMap<SocketAddr, UdpSocketHandle> = HashMap::new();

        loop {
            let mut buf = vec![0u8; 65536];
            select! {
                Ok((size, addr)) = socket.recv_from(&mut buf) => {
                    buf.truncate(size);

                    match map.get_mut(&addr) {
                        Some(handle) => {
                            handle.sender.send(buf).await;
                        },
                        None => {
                            let (socket, handle) = UdpStream::new(addr, out_packet_tx.clone());
                            map.insert(addr, handle);
                            if socket_tx.send(socket).await.is_err() {
                                break
                            }
                        },
                    }
                },
                Some((packet, addr)) = out_packet_rx.recv() => {
                    let ret = socket.send_to(&packet, &addr).await;
                    // let _ = callback.send(ret);
                },
                else => (),
            }
        }
    }
}

struct UdpSocketHandle {
    sender: mpsc::Sender<UdpItem>,
}

pub struct UdpStream {
    addr: SocketAddr,
    receiver: mpsc::Receiver<UdpItem>,
    out_packet_tx: mpsc::Sender<SendTo>,
}

impl UdpStream {
    fn new(addr: SocketAddr, out_packet_tx: mpsc::Sender<SendTo>) -> (UdpStream, UdpSocketHandle) {
        let (sender, receiver) = mpsc::channel::<UdpItem>(10);
        (UdpStream {
            addr,
            receiver,
            out_packet_tx,
        }, UdpSocketHandle { sender })
    }
    pub async fn send(&self, buf: UdpItem) -> tokio::io::Result<()> {
        match self.out_packet_tx.clone().send((buf, self.addr)).await {
            Ok(_) => Ok(()),
            Err(_) => Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "failed to send"))
        }
    }
}

impl Stream for UdpStream {
    type Item = UdpItem;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}
