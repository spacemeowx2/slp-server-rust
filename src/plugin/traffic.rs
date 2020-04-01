use crate::slp::plugin::*;
use crate::slp::spawn_stream;
use serde::Serialize;
use juniper::GraphQLObject;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

/// Traffic infomation
#[derive(Clone, Debug, Eq, PartialEq, Serialize, GraphQLObject)]
pub struct TrafficInfo {
    /// upload bytes last second
    upload: i32,
    /// download bytes last second
    download: i32,
}

impl TrafficInfo {
    fn new() -> Self {
        Self {
            upload: 0,
            download: 0,
        }
    }
    fn upload(&mut self, size: i32) {
        self.upload += size
    }
    fn download(&mut self, size: i32) {
        self.download += size
    }
}

#[derive(Clone)]
struct Inner(Arc<Mutex<(TrafficInfo, TrafficInfo)>>);

impl Inner {
    fn new() -> Inner {
        Inner(Arc::new(Mutex::new((TrafficInfo::new(), TrafficInfo::new()))))
    }
    async fn clear_traffic(&mut self) -> TrafficInfo {
        let mut inner = self.0.lock().await;
        inner.1 = std::mem::replace(&mut inner.0, TrafficInfo::new());
        inner.1.clone()
    }
    async fn in_packet(&mut self, packet: &InPacket) {
        self.0.lock().await.0.download(packet.as_ref().len() as i32)
    }
    async fn out_packet(&mut self, packet: &OutPacket) {
        self.0.lock().await.0.upload(packet.as_ref().len() as i32)
    }
}

pub struct Traffic(Inner, broadcast::Sender<TrafficInfo>);

impl Traffic {
    fn new() -> Traffic {
        let inner = Inner::new();

        let traffic_sender = spawn_stream(&inner, |mut inner| async move {
            inner.clear_traffic().await
        });

        Traffic(inner, traffic_sender)
    }
}

#[async_trait]
impl Plugin for Traffic {
    async fn in_packet(&mut self, packet: &InPacket) {
        self.0.in_packet(packet).await
    }
    async fn out_packet(&mut self, packet: &OutPacket) {
        self.0.out_packet(packet).await
    }
}

pub struct Factory;

impl PluginFactory for Factory {
    fn name(&self) -> String {
        "traffic".to_string()
    }
    fn new(&self, _: Context) -> Box<dyn Plugin + Send + 'static> {
        Box::new(Traffic::new())
    }
}
