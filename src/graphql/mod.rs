mod model;

use async_std::sync::{Arc, RwLock};
use super::slp::UDPServer;
pub use model::{QueryRoot, SubscriptionRoot};

#[derive(Clone)]
pub struct Data {
  udp_server: Arc<RwLock<UDPServer>>,
}

impl Data {
  pub fn new(udp_server: UDPServer) -> Self {
    Data {
      udp_server: Arc::new(RwLock::new(udp_server)),
    }
  }
  pub async fn online(&self) -> i32 {
    self.udp_server.read().await.online()
  }
}
