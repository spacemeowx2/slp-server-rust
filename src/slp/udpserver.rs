use super::{
    log_warn, spawn_stream, BoxPlugin, BoxPluginType, Context, Event, ForwarderFrame, Packet,
    Parser, PeerManager, PeerManagerInfo,
    BoxedAuthProvider,
};
use super::{packet_stream, PacketReceiver, PacketSender};
use crate::util::{create_socket, FilterSameExt};
use async_graphql::SimpleObject;
use futures::prelude::*;
use futures::stream::{BoxStream, StreamExt};
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::net::udp::{SendHalf, RecvHalf};

type ServerInfoStream = BoxStream<'static, ServerInfo>;

/// Infomation about this server
#[SimpleObject]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ServerInfo {
    /// The number of online clients
    online: i32,
    /// The number of idle clients(not sending packets for 30s)
    idle: i32,
    /// The version of the server
    version: String,
}

pub struct UDPServerConfig {
    ignore_idle: bool,
    auth_provider: Option<BoxedAuthProvider>,
}

struct Endpoint<'a> {
    server: &'a UDPServer,
}

struct UDPServer {
    rx: Mutex<RecvHalf>,
    tx: Mutex<SendHalf>,
}

impl UDPServer {
    pub async fn new(addr: &SocketAddr, config: UDPServerConfig) -> Result<Self> {
        let (rx, tx) = create_socket(addr).await?.split();
        tokio::spawn(async {

        });
        Ok(UDPServer {
            rx: Mutex::new(rx),
            tx: Mutex::new(tx),
        })
    }
    pub async fn serve(&self) -> Result<()> {
        let mut socket = self.rx.try_lock()?;
        loop {
            let mut buf = vec![0u8; 65536];
            let (size, addr) = socket.recv_from(&mut buf).await?;
            buf.truncate(size);

            let a = (buf, addr);
        }
    }
}

#[tokio::test]
async fn test() -> Result<()> {
    let server = UDPServer::new(&"127.0.0.1:12345".parse().unwrap(), UDPServerConfig {
        ignore_idle: false,
        auth_provider: None,
    }).await?;

    server.serve().await?;

    Ok(())
}
