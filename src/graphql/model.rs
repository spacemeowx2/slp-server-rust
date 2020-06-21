use anyhow::anyhow;
use async_graphql::{EmptyMutation, EmptySubscription, FieldError, Value, FieldResult, Context};
use crate::slp::{UDPServer, ServerInfo};
use crate::plugin::traffic::{TrafficInfo, TRAFFIC_TYPE};
use crate::plugin::ldn_mitm::{LDN_MITM_TYPE, RoomInfo};
use futures::stream::BoxStream;
use std::sync::Arc;
use async_graphql::*;

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
            config: Arc::new(Config {
                admin_token
            })
        }
    }
}

type TrafficInfoStream = BoxStream<'static, FieldResult<TrafficInfo>>;
pub struct Query;

#[Object]
impl Query {
    /// Infomation about this server
    async fn server_info(&self, ctx: &Context<'_>) -> ServerInfo {
        ctx.data::<Ctx>().udp_server.server_info().await
    }
    /// Traffic infomation last second
    async fn traffic_info(&self, ctx: &Context<'_>, token: String) -> FieldResult<TrafficInfo> {
        let ctx = ctx.data::<Ctx>();
        if Some(token) == ctx.config.admin_token {
            let r = ctx.udp_server
                .get_plugin(&TRAFFIC_TYPE, |traffic| {
                    traffic.map(Clone::clone)
                })
                .await
                .ok_or("This plugin is not available")?;
            Ok(r.traffic_info().await)
        } else {
            Err("Permission denied".into())
        }
    }
    /// Current rooms
    async fn room(&self, ctx: &Context<'_>) -> FieldResult<Vec<RoomInfo>> {
        let ctx = ctx.data::<Ctx>();
        let r = ctx.udp_server
            .get_plugin(&LDN_MITM_TYPE, |ldn_mitm| {
                ldn_mitm.map(|i| i.room_info())
            })
            .await
            .ok_or("This plugin is not available")?;
        let r = r.lock().await;
        Ok(r.iter().map(|(_, v)| v.clone()).collect())
    }
}

type ServerInfoStream = BoxStream<'static, FieldResult<ServerInfo>>;

// pub struct Subscription;

// #[Object]
// impl Subscription {
//     /// Infomation about this server
//     async fn server_info(&self, context: &Context<'_>) -> ServerInfoStream {
//         let context = context.clone();

//         context.udp_server
//             .server_info_stream()
//             .await
//             .map(|info| Ok(info))
//             .boxed()
//     }
//     /// Traffic infomation last second
//     async fn traffic_info(&self, context: &Context<'_>, token: String) -> Result<TrafficInfoStream, FieldError> {
//         if Some(token) == context.config.admin_token {
//             let r = context.udp_server
//                 .get_plugin(&TRAFFIC_TYPE, |traffic| {
//                     traffic.map(Clone::clone)
//                 })
//                 .await;
//             match r {
//                 Some(r) => Ok(r
//                     .traffic_info_stream()
//                     .await
//                     .map(|info| Ok(info))
//                     .boxed()
//                 ),
//                 None => Err(FieldError::new("This plugin is not available", Value::null())),
//             }
//             // let context = context.clone();

//             // Ok(context.udp_server
//             //     .traffic_info_stream()
//             //     .await
//             //     .map(|info| Ok(info))
//             //     .boxed())
//         } else {
//             Err(FieldError::new("Permission denied", Value::null()))
//         }
//     }
// }

type RootSchema = Schema<Query, EmptyMutation, EmptySubscription>;

pub fn schema(ctx: &Ctx) -> RootSchema {
    Schema::build(
        Query,
        EmptyMutation,
        EmptySubscription,
    )
    .data(ctx.clone())
    .finish()
}
