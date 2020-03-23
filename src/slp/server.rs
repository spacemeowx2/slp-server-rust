use async_std::io::Result;
use async_std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use async_std::sync::{Arc, RwLock};
use lru::LruCache;

struct Peer {}

pub struct UDPServer {
    cache: LruCache<SocketAddr, Peer>,
}
pub type SharedUDPServer = Arc<RwLock<UDPServer>>;

impl UDPServer {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(100),
        }
    }
    pub async fn new2<A: ToSocketAddrs>(addrs: A) -> Result<Self> {
        Ok(Self {
            cache: LruCache::new(100),
        })
    }
    pub async fn serve(&mut self) -> Result<()> {
        Ok(())
    }
    pub fn online(&self) -> i32 {
        0
    }
}
