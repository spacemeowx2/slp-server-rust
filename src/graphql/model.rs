use juniper::{EmptyMutation, FieldError, RootNode, Value};
use crate::slp::{UDPServer, ServerInfo, TrafficInfo};
use futures::stream::BoxStream;
use std::sync::Arc;

pub struct Config {
    pub admin_token: Option<String>,
}

#[derive(Clone)]
pub struct Context {
    pub udp_server: UDPServer,
    pub config: Arc<Config>,
}
impl juniper::Context for Context {}

impl Context {
    pub fn new(udp_server: UDPServer, admin_token: Option<String>) -> Self {
        Self {
            udp_server,
            config: Arc::new(Config {
                admin_token
            })
        }
    }
}

type TrafficInfoStream = BoxStream<'static, Result<TrafficInfo, FieldError>>;
pub struct Query;

#[juniper::graphql_object(Context = Context)]
impl Query {
    /// Infomation about this server
    async fn server_info(context: &Context) -> ServerInfo {
        context.udp_server.server_info().await
    }
    /// Traffic infomation last second
    async fn traffic_info(context: &Context, token: String) -> Result<TrafficInfo, FieldError> {
        if Some(token) == context.config.admin_token {
            Ok(context.udp_server.traffic_info().await)
        } else {
            Err(FieldError::new("Permission denied", Value::null()))
        }
    }
}

type ServerInfoStream = BoxStream<'static, Result<ServerInfo, FieldError>>;

pub struct Subscription;

#[juniper::graphql_subscription(Context = Context)]
impl Subscription {
    /// Infomation about this server
    async fn server_info(context: &Context) -> ServerInfoStream {
        let context = context.clone();

        context.udp_server
            .server_info_stream()
            .await
            .map(|info| Ok(info))
            .boxed()
    }
    /// Traffic infomation last second
    async fn traffic_info(context: &Context, token: String) -> Result<TrafficInfoStream, FieldError> {
        if Some(token) == context.config.admin_token {
            let context = context.clone();

            Ok(context.udp_server
                .traffic_info_stream()
                .await
                .map(|info| Ok(info))
                .boxed())
        } else {
            Err(FieldError::new("Permission denied", Value::null()))
        }
    }
}

type Schema = RootNode<'static, Query, EmptyMutation<Context>, Subscription>;

pub fn schema() -> Schema {
    Schema::new(
        Query,
        EmptyMutation::<Context>::new(),
        Subscription,
    )
}
