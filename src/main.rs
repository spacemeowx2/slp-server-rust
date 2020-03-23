mod graphql;
mod slp;

use actix_web::{web, App, HttpServer};
use async_graphql::{EmptyMutation, Schema};
use slp::UDPServer;
use async_std::sync::{Arc, RwLock};
use async_std::task;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
  let port: u16 = 11451;
  let bind_address = format!("{}:{}", "0.0.0.0", port);

  HttpServer::new(move || {
    let udp_server = Arc::new(RwLock::new(UDPServer::new()));
    let schema =
      Schema::new(graphql::QueryRoot, EmptyMutation, graphql::SubscriptionRoot)
      .data(udp_server);
    let handler = async_graphql_actix_web::HandlerBuilder::new(schema)
      .enable_ui(&format!("http://localhost:{}", port), Some(&format!("ws://localhost:{}", port)))
      .enable_subscription()
      .build();
    App::new().service(web::resource("/").to(handler))
  })
  .bind(&bind_address)?
  .run()
  .await
}
