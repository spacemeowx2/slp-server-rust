use juniper::{EmptyMutation, FieldError, RootNode};
use crate::slp::{UDPServer, ServerInfo};
use futures::stream::BoxStream;

#[derive(Clone)]
pub struct Context {
    pub udp_server: UDPServer,
}
impl juniper::Context for Context {}

pub struct Query;

#[juniper::graphql_object(Context = Context)]
impl Query {
    /// Infomation about this server
    async fn server_info(context: &Context) -> ServerInfo {
        context.udp_server.server_info().await
    }
}

type ServerInfoStream = BoxStream<'static, Result<ServerInfo, FieldError>>;

pub struct Subscription;

#[juniper::graphql_subscription(Context = Context)]
impl Subscription {
    /// Infomation about this server
    async fn server_info(context: &Context) -> ServerInfoStream {
        let context = context.clone();
        let state: Option<ServerInfo> = None;

        context.udp_server
            .server_info_stream()
            .await
            .map(|info| Ok(info))
            .boxed()
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
