use juniper::{EmptyMutation, EmptySubscription, RootNode};

#[derive(Clone)]
pub struct Context {}
impl juniper::Context for Context {}

struct ServerInfo {}

/// Infomation about this server
#[juniper::graphql_object(
    Context = Context,
)]
impl ServerInfo {
    /// The number of online clients
    fn online(&self) -> i32 {
        0
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

type Schema = RootNode<'static, Query, EmptyMutation<Context>, EmptySubscription<Context>>;

pub fn schema() -> Schema {
    Schema::new(
        Query,
        EmptyMutation::<Context>::new(),
        EmptySubscription::<Context>::new(),
    )
}
