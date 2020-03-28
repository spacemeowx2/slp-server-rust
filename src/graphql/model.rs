use juniper::{EmptyMutation, FieldError, RootNode};
use crate::slp::{UDPServer, ServerInfo};
use std::time::Duration;
use futures::stream::BoxStream;
use super::filter_same::FilterSameExt;

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

        tokio::time::interval(
            Duration::from_secs(1)
        )
        .then(move |_| {
            let context = context.clone();
            async move {
                context.udp_server.server_info().await
            }
        })
        .filter_same()
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
