mod graphql;

use actix_web::{web, App, HttpServer};
use async_graphql::{EmptyMutation, Schema};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
  HttpServer::new(move || {
      let schema =
          Schema::new(graphql::QueryRoot, EmptyMutation, graphql::SubscriptionRoot).data(graphql::Data::new());
      let handler = async_graphql_actix_web::HandlerBuilder::new(schema)
          .enable_ui("http://localhost:8010", Some("ws://localhost:8010"))
          .enable_subscription()
          .build();
      App::new().service(web::resource("/").to(handler))
  })
  .bind("127.0.0.1:8010")?
  .run()
  .await
}
