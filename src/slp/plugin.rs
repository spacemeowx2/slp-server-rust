pub use super::{PeerManager, InPacket, OutPacket};
pub use async_trait::async_trait;

pub struct Context<'a> {
    peer_manager: &'a PeerManager,
}

impl<'a> Context<'a> {
    pub fn new(peer_manager: &'a PeerManager) -> Self {
        Self {
            peer_manager,
        }
    }
}

pub trait PluginFactory {
    fn name(&self) -> String;
    fn new(&self, context: Context) -> Box<dyn Plugin + Send + 'static>;
}
pub type BoxPluginFactory = Box<dyn PluginFactory + Send + Sync + 'static>;

#[async_trait]
pub trait Plugin {
    async fn in_packet(&mut self, packet: &InPacket) {}
    async fn out_packet(&mut self, packet: &OutPacket) {}
}

pub type BoxPlugin = Box<dyn Plugin + Send + 'static>;
