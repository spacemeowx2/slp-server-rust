#[cfg(feature = "ldn_mitm")]
pub mod ldn_mitm;
pub mod traffic;

use crate::slp::{BoxPluginType, UDPServer};
use lazy_static::lazy_static;

lazy_static! {
    static ref PLUGINS: Vec<BoxPluginType> = {
        let mut plugins: Vec<BoxPluginType> = vec![];
        if cfg!(feature = "ldn_mitm") {
            plugins.push(Box::new(ldn_mitm::LdnMitmType));
        }
        plugins.push(Box::new(traffic::TrafficType));
        plugins
    };
}

pub async fn register_plugins(server: &UDPServer) {
    for p in PLUGINS.iter() {
        server.add_plugin(p).await;
    }
}
