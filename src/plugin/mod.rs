#[cfg(feature = "ldn_mitm")]
mod ldn_mitm;
mod traffic;

use crate::slp::{UDPServer, BoxPluginFactory};
use lazy_static::lazy_static;

lazy_static! {
    static ref PLUGINS: Vec<BoxPluginFactory> = {
        let mut plugins: Vec<BoxPluginFactory> = vec![];
        if cfg!(feature = "ldn_mitm") {
            plugins.push(Box::new(ldn_mitm::Factory));
        }
        plugins.push(Box::new(traffic::Factory));
        plugins
    };
}

pub async fn register_plugins(server: &UDPServer) {
    for p in PLUGINS.iter() {
        server.add_plugin(p).await;
    }
}
