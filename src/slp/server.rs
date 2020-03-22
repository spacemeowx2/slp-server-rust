use lru::LruCache;
use async_std::net::{UdpSocket, SocketAddr, ToSocketAddrs};
use async_std::io::Result;

struct Peer {

}

pub struct UDPServer {
  cache: LruCache<SocketAddr, Peer>,
}

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
