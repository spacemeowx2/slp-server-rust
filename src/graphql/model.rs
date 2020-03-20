use super::Data;
use async_graphql::Context;

pub struct ServerInfo {
}

#[async_graphql::Object(desc = "Infomation about this server")]
impl ServerInfo {
  #[field(desc = "The number of online clients")]
  async fn online(&self, ctx: &Context<'_>) -> i32 {
    ctx.data::<Data>().online
  }
}

pub struct QueryRoot;

#[async_graphql::Object(desc = "Queryroot")]
impl QueryRoot {
  #[field(desc = "Infomation about this server")]
  async fn server_info(&self, _ctx: &Context<'_>) -> ServerInfo {
    ServerInfo{}
  }
}

pub struct SubscriptionRoot;

#[async_graphql::Subscription]
impl SubscriptionRoot {
    #[field]
    fn server_info(&self, _server_info: &ServerInfo) -> bool {
        true
    }
}
