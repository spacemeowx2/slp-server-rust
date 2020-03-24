use juniper::{EmptyMutation, FieldError, RootNode};
use crate::slp::UDPServer;
use std::{pin::Pin, time::Duration};
use futures::Stream;
use super::filter_same::FilterSameExt;

#[derive(Clone)]
pub struct Context {
    pub udp_server: UDPServer,
}
impl juniper::Context for Context {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ServerInfo {
    online: i32,
}

impl ServerInfo {
    async fn new(context: &Context) -> ServerInfo {
        ServerInfo {
            online: context.udp_server.online().await
        }
    }
}

/// Infomation about this server
#[juniper::graphql_object(
    Context = Context,
)]
impl ServerInfo {
    /// The number of online clients
    async fn online(&self) -> i32 {
        self.online
    }
    /// The version of the server
    fn version() -> &str {
        std::env!("CARGO_PKG_VERSION")
    }
}

pub struct Query;

#[juniper::graphql_object(Context = Context)]
impl Query {
    /// Infomation about this server
    async fn server_info(context: &Context) -> ServerInfo {
        ServerInfo::new(context).await
    }
}

type ServerInfoStream = Pin<Box<dyn Stream<Item = Result<ServerInfo, FieldError>> + Send>>;

pub struct Subscription;

#[juniper::graphql_subscription(Context = Context)]
impl Subscription {
    /// Infomation about this server
    async fn server_info(context: &Context) -> ServerInfoStream {
        let context = context.clone();
        let state: Option<ServerInfo> = None;

        let stream = tokio::time::interval(
            Duration::from_secs(1)
        )
        .then(move |_| {
            let context = context.clone();
            async move {
                ServerInfo::new(&context.clone()).await
            }
        })
        .filter_same()
        .map(|info| Ok(info));

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
