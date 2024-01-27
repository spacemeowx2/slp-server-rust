use super::{Event, OutAddr, Peer};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::{net::UdpSocket, sync::mpsc};

pub struct PeerManagerInfo {
    /// The number of online clients
    pub online: i32,
    /// The number of idle clients(not sending packets for 30s)
    pub idle: i32,
}

struct InnerPeerManager {
    /// real ip to peer map
    cache: HashMap<SocketAddr, Peer>,
    /// key is the inner ip in virtual LAN, value is cache's key
    /// a client may have more than one inner ip.
    map: HashMap<Ipv4Addr, SocketAddr>,

    ignore_idle: bool,
}

impl InnerPeerManager {
    fn new(ignore_idle: bool) -> Self {
        Self {
            cache: HashMap::new(),
            map: HashMap::new(),
            ignore_idle,
        }
    }
}

#[derive(Clone)]
pub struct PeerManager {
    udp_socket: Arc<UdpSocket>,
    inner: Arc<Mutex<InnerPeerManager>>,
}

impl PeerManager {
    pub fn new(udp_socket: Arc<UdpSocket>, ignore_idle: bool) -> Self {
        Self {
            udp_socket,
            inner: Arc::new(Mutex::new(InnerPeerManager::new(ignore_idle))),
        }
    }
    pub async fn remove(&self, addr: &SocketAddr) {
        let cache = &mut self.inner.lock().cache;
        cache.remove(&addr);
    }
    pub async fn peer_mut<F>(&self, addr: &SocketAddr, event_send: &mpsc::Sender<Event>, func: F)
    where
        F: FnOnce(&mut Peer) -> (),
    {
        let cache = &mut self.inner.lock().cache;
        let peer = cache
            .entry(*addr)
            .or_insert_with(|| Peer::new(*addr, event_send.clone()));
        func(peer)
    }
    pub async fn send_broadcast(&self, packet: &[u8]) -> std::io::Result<usize> {
        let addrs = {
            let inner = &mut self.inner.lock();
            inner
                .cache
                .iter()
                .map(|(addr, _)| *addr)
                .collect::<Vec<_>>()
        };
        self.send_lan(packet, addrs).await
    }
    pub async fn get_dest_sockaddr(&self, from: SocketAddr, out_addr: OutAddr) -> Vec<SocketAddr> {
        let inner = &mut self.inner.lock();
        inner.map.insert(*out_addr.src_ip(), from);
        if let Some(addr) = inner.map.get(&out_addr.dst_ip()) {
            vec![*addr]
        } else {
            let addrs = inner
                .cache
                .iter()
                .filter(|(_, i)| !inner.ignore_idle || i.state.is_connected())
                .filter(|(addr, _)| &&from != addr)
                .map(|(addr, _)| *addr)
                .collect::<Vec<_>>();
            addrs
        }
    }
    pub async fn send_lan(&self, packet: &[u8], addrs: Vec<SocketAddr>) -> std::io::Result<usize> {
        let len = packet.len();
        let size: usize = addrs.len() * len;
        for addr in addrs {
            // TODO: handle error
            let _ = self.udp_socket.send_to(&packet, addr).await;
        }
        Ok(size)
    }
    pub async fn server_info(&self) -> PeerManagerInfo {
        let inner = &self.inner.lock();
        let online = inner.cache.len() as i32;
        let idle = inner.cache.values().filter(|i| i.state.is_idle()).count() as i32;
        PeerManagerInfo { online, idle }
    }
}
