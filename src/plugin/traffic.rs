use crate::slp::plugin::*;
use crate::slp::spawn_stream;
use crate::util::FilterSameExt;
use async_graphql::SimpleObject;
use futures::prelude::*;
use futures::{future, stream::BoxStream};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// Traffic infomation
#[SimpleObject]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TrafficInfo {
    /// upload bytes last second
    upload: i32,
    /// download bytes last second
    download: i32,
    /// upload packets last second
    upload_packet: i32,
    /// download packets last second
    download_packet: i32,
}
type TrafficInfoStream = BoxStream<'static, TrafficInfo>;

impl TrafficInfo {
    fn new() -> Self {
        Self {
            upload: 0,
            download: 0,
            upload_packet: 0,
            download_packet: 0,
        }
    }
    fn on_upload(&mut self, size: i32) {
        self.upload += size;
        self.upload_packet += 1;
    }
    fn on_download(&mut self, size: i32) {
        self.download += size;
        self.download_packet += 1;
    }
}

#[derive(Clone, Debug)]
struct Inner(Arc<Mutex<(TrafficInfo, TrafficInfo)>>);

impl Inner {
    fn new() -> Inner {
        Inner(Arc::new(Mutex::new((
            TrafficInfo::new(),
            TrafficInfo::new(),
        ))))
    }
    async fn clear_traffic(&mut self) -> TrafficInfo {
        let mut inner = self.0.lock().await;
        inner.1 = std::mem::replace(&mut inner.0, TrafficInfo::new());
        inner.1.clone()
    }
    async fn in_packet(&mut self, packet: &InPacket) {
        self.0
            .lock()
            .await
            .0
            .on_download(packet.as_ref().len() as i32)
    }
    async fn out_packet(&mut self, packet: &Packet, addrs: &[SocketAddr]) {
        self.0
            .lock()
            .await
            .0
            .on_upload((packet.len() * addrs.len()) as i32)
    }
    async fn traffic_info(&self) -> TrafficInfo {
        self.0.lock().await.0.clone()
    }
}

#[derive(Clone, Debug)]
pub struct Traffic(Inner, broadcast::Sender<TrafficInfo>);

impl Traffic {
    fn new() -> Self {
        let inner = Inner::new();

        let traffic_sender =
            spawn_stream(
                &inner,
                |mut inner| async move { inner.clear_traffic().await },
            );

        Self(inner, traffic_sender)
    }
    pub async fn traffic_info(&self) -> TrafficInfo {
        self.0.traffic_info().await
    }
    pub async fn traffic_info_stream(&self) -> TrafficInfoStream {
        let stream = self
            .1
            .subscribe()
            .take_while(|info| future::ready(info.is_ok()))
            .map(|info| info.unwrap());

        stream::once(future::ready(self.traffic_info().await))
            .chain(stream)
            .filter_same()
            .boxed()
    }
}

#[async_trait]
impl Plugin for Traffic {
    async fn in_packet(&mut self, packet: &InPacket) -> Result<(), ()> {
        self.0.in_packet(packet).await;
        Ok(())
    }
    async fn out_packet(&mut self, packet: &Packet, addrs: &[SocketAddr]) -> Result<(), ()> {
        self.0.out_packet(packet, addrs).await;
        Ok(())
    }
}

pub struct TrafficType;
pub const TRAFFIC_NAME: &str = "traffic";

lazy_static! {
    pub static ref TRAFFIC_TYPE: BoxPluginType<Traffic> = Box::new(TrafficType);
}

impl PluginType<Traffic> for TrafficType {
    fn name(&self) -> String {
        TRAFFIC_NAME.to_string()
    }
    fn new(&self, _: Context) -> BoxPlugin {
        Box::new(Traffic::new())
    }
}

impl PluginType for TrafficType {
    fn name(&self) -> String {
        TRAFFIC_NAME.to_string()
    }
    fn new(&self, _: Context) -> BoxPlugin {
        Box::new(Traffic::new())
    }
}

#[tokio::test]
async fn get_traffic_from_box() {
    let p: BoxPlugin = Box::new(Traffic::new());
    let t = p.as_any().downcast_ref::<Traffic>();
    assert!(t.is_some(), true);
}
