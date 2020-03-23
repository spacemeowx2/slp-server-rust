use async_std::io::Result;
use async_std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use async_std::sync::{Arc, RwLock};
use lru::LruCache;

struct Peer {}

struct InnerServer {
    cache: LruCache<SocketAddr, Peer>,
}
impl InnerServer {
    fn new() -> Self {
        Self {
            cache: LruCache::new(100),
        }
    }
}

#[derive(Clone)]
pub struct UDPServer {
    inner: Arc<RwLock<InnerServer>>,
}

impl UDPServer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(InnerServer::new())),
        }
    }
    pub async fn serve(&mut self) -> Result<()> {
        Ok(())
    }
    pub fn online(&self) -> i32 {
        0
    }
}
