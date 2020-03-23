use juniper::{EmptyMutation, FieldError, RootNode};
use crate::slp::UDPServer;
use std::{pin::Pin, time::Duration};
use futures::Stream;

#[derive(Clone)]
pub struct Context {
    pub udp_server: UDPServer,
}
impl juniper::Context for Context {}

struct ServerInfo {}

/// Infomation about this server
#[juniper::graphql_object(
    Context = Context,
)]
impl ServerInfo {
    /// The number of online clients
    async fn online(context: &Context) -> i32 {
        context.udp_server.online().await
    }
    /// The version of the server
    async fn version() -> &str {
        std::env!("CARGO_PKG_VERSION")
    }
}

pub struct Query;

#[juniper::graphql_object(Context = Context)]
impl Query {
    /// Infomation about this server
    async fn server_info() -> ServerInfo {
        ServerInfo {}
    }
}

type ServerInfoStream = Pin<Box<dyn Stream<Item = Result<ServerInfo, FieldError>> + Send>>;

pub struct Subscription;

#[juniper::graphql_subscription(Context = Context)]
impl Subscription {
    /// Infomation about this server
    async fn server_info() -> ServerInfoStream {
        let stream = tokio::time::interval(
            Duration::from_secs(5)
        ).map(move |_| {
            Ok(ServerInfo {})
        });

        Box::pin(stream)
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
