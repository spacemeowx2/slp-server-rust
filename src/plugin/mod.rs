pub mod blocker;
#[cfg(feature = "ldn_mitm")]
pub mod ldn_mitm;
pub mod traffic;

use crate::slp::{BoxPluginType, UDPServer};
use once_cell::sync::OnceCell;

fn plugins() -> &'static Vec<BoxPluginType> {
    static PLUGINS: OnceCell<Vec<BoxPluginType>> = OnceCell::new();
    PLUGINS.get_or_init(|| {
        let mut plugins: Vec<BoxPluginType> = vec![];
        if cfg!(feature = "ldn_mitm") {
            plugins.push(Box::new(ldn_mitm::LdnMitmType));
        }
        plugins.push(Box::new(traffic::TrafficType));
        plugins.push(Box::new(blocker::BlockerType));
        plugins
    })
}

pub async fn register_plugins(server: &UDPServer) {
    for p in plugins().iter() {
        server.add_plugin(p).await;
    }
}
