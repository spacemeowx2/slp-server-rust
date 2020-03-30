use tokio::io::Result;
use tokio::net::udp::SendHalf;
use tokio::sync::{RwLock, mpsc};
use std::net::{SocketAddr, Ipv4Addr};
use std::sync::Arc;
use super::{Event, Peer, Packet};
use std::collections::HashMap;

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
    fn new(ignore_idle: bool,) -> Self {
        Self {
            cache: HashMap::new(),
            map: HashMap::new(),
            ignore_idle,
        }
    }
}

#[derive(Clone)]
pub struct PeerManager {
    inner: Arc<RwLock<InnerPeerManager>>,
}

impl PeerManager {
    pub fn new(ignore_idle: bool) -> Self {
        Self {
            inner: Arc::new(RwLock::new(
                InnerPeerManager::new(ignore_idle)
            )),
        }
    }
    pub async fn remove(&self, addr: &SocketAddr) {
        let cache = &mut self.inner.write().await.cache;
        cache.remove(&addr);
    }
    pub async fn peer_mut<F>(&self, addr: SocketAddr, event_send: &mpsc::Sender<Event>, func: F)
    where
        F: FnOnce(&mut Peer) -> ()
    {
        let cache = &mut self.inner.write().await.cache;
        let peer = {
            if cache.get(&addr).is_none() {
                cache.insert(
                    addr,
                    Peer::new(addr, event_send.clone())
                );
            }
            cache.get_mut(&addr).unwrap()
        };
        func(peer)
    }
    pub async fn send_lan(
        &self,
        send_half: &mut SendHalf,
        packet: Packet,
        from: SocketAddr,
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
    ) -> Result<usize>
    {
        let inner = &mut self.inner.write().await;
        inner.map.insert(src_ip, from);
        if let Some(addr) = inner.map.get(&dst_ip) {
            Ok(send_half.send_to(&packet, &addr).await?)
        } else {
            let mut size: usize = 0;
            for (addr, _) in inner.cache.iter()
                .filter(|(_, i)| !inner.ignore_idle || i.state.is_connected())
                .filter(|(addr, _) | &&from != addr)
            {
                size += send_half.send_to(&packet, &addr).await?;
            }
            Ok(size)
        }
    }
    pub async fn server_info(&self) -> PeerManagerInfo {
        let inner = &self.inner.read().await;
        let online = inner.cache.len() as i32;
        let idle = inner.cache.values().filter(|i| i.state.is_idle()).count() as i32;
        PeerManagerInfo {
            online,
            idle,
        }
    }
}
