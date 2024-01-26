pub mod blocker;
#[cfg(feature = "ldn_mitm")]
pub mod ldn_mitm;
pub mod traffic;

use crate::slp::UDPServer;

pub async fn register_plugins(server: &UDPServer) {
    if cfg!(feature = "ldn_mitm") {
        server.add_plugin::<ldn_mitm::LdnMitmPlugin>().await;
    }
    server.add_plugin::<traffic::TrafficPlugin>().await;
    server.add_plugin::<blocker::BlockerPlugin>().await;
}
