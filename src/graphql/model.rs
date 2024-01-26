use crate::plugin::ldn_mitm::{RoomInfo, LDN_MITM_TYPE};
use crate::plugin::traffic::{TrafficInfo, TRAFFIC_TYPE};
use crate::slp::{ServerInfo, UDPServer};
use async_graphql::{Context, EmptyMutation, FieldResult, Object, Schema, Subscription};
use futures::stream::BoxStream;
use std::sync::Arc;

#[derive(Debug)]
struct StringError(&'static str);
impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for StringError {}

pub struct Config {
    pub admin_token: Option<String>,
}

#[derive(Clone)]
pub struct Ctx {
    pub udp_server: UDPServer,
    pub config: Arc<Config>,
}

impl Ctx {
    pub fn new(udp_server: UDPServer, admin_token: Option<String>) -> Self {
        Self {
            udp_server,
            config: Arc::new(Config { admin_token }),
        }
    }
}

pub struct Query;

#[Object]
impl Query {
    /// Infomation about this server
    async fn server_info(&self, ctx: &Context<'_>) -> FieldResult<ServerInfo> {
        Ok(ctx.data::<Ctx>()?.udp_server.server_info().await)
    }
    /// Traffic infomation last second
    async fn traffic_info(&self, ctx: &Context<'_>, token: String) -> FieldResult<TrafficInfo> {
        let ctx = ctx.data::<Ctx>()?;
        if Some(token) == ctx.config.admin_token {
            let r = ctx
                .udp_server
                .get_plugin(&TRAFFIC_TYPE, |traffic| traffic.map(|t| t.clone()))
                .await
                .ok_or("This plugin is not available")?;
            Ok(r.traffic_info().await)
        } else {
            Err("Permission denied".into())
        }
    }
    /// Current rooms
    async fn room(&self, ctx: &Context<'_>) -> FieldResult<Vec<RoomInfo>> {
        let ctx = ctx.data::<Ctx>()?;
        let r = ctx
            .udp_server
            .get_plugin(&LDN_MITM_TYPE, |ldn_mitm| ldn_mitm.map(|i| i.room_info()))
            .await
            .ok_or("This plugin is not available")?;
        let r = r.lock().await;
        Ok(r.iter().map(|(_, v)| v.clone()).collect())
    }
}

type ServerInfoStream = BoxStream<'static, ServerInfo>;
type TrafficInfoStream = BoxStream<'static, TrafficInfo>;

pub struct Subscription;

#[Subscription]
impl Subscription {
    /// Infomation about this server
    async fn server_info(&self, context: &Context<'_>) -> FieldResult<ServerInfoStream> {
        let context = context.data::<Ctx>()?.clone();

        Ok(context.udp_server.server_info_stream().await)
    }
    /// Traffic infomation last second
    async fn traffic_info(
        &self,
        context: &Context<'_>,
        token: String,
    ) -> FieldResult<TrafficInfoStream> {
        let context = context.data::<Ctx>()?.clone();

        if Some(token) == context.config.admin_token {
            let r = context
                .udp_server
                .get_plugin(&TRAFFIC_TYPE, |traffic| traffic.map(|t| t.clone()))
                .await
                .ok_or("This plugin is not available")?;
            Ok(r.traffic_info_stream().await)
        } else {
            Err("Permission denied".into())
        }
    }
}

pub type SlpServerSchema = Schema<Query, EmptyMutation, Subscription>;

pub fn schema(ctx: &Ctx) -> SlpServerSchema {
    Schema::build(Query, EmptyMutation, Subscription)
        .data(ctx.clone())
        .finish()
}
