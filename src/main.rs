mod graphql;

use actix_web::{guard, web, App, HttpResponse, HttpServer};
use async_graphql::http::{graphiql_source, playground_source, GQLRequest, GQLResponse};
use async_graphql::{EmptyMutation, EmptySubscription, Schema};

type GraphqlSchema = Schema<graphql::QueryRoot, EmptyMutation, EmptySubscription>;

async fn index(s: web::Data<GraphqlSchema>, req: web::Json<GQLRequest>) -> web::Json<GQLResponse> {
  web::Json(req.into_inner().execute(&s).await)
}

async fn gql_playgound() -> HttpResponse {
  HttpResponse::Ok()
      .content_type("text/html; charset=utf-8")
      .body(playground_source("/", None))
}

async fn gql_graphiql() -> HttpResponse {
  HttpResponse::Ok()
      .content_type("text/html; charset=utf-8")
      .body(graphiql_source("/"))
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
  HttpServer::new(move || {
      App::new()
          .data(
              Schema::new(graphql::QueryRoot{}, EmptyMutation, EmptySubscription)
                  .data(graphql::Data::new()),
          )
          .service(web::resource("/").guard(guard::Post()).to(index))
          .service(web::resource("/").guard(guard::Get()).to(gql_playgound))
          .service(
              web::resource("/graphiql")
                  .guard(guard::Get())
                  .to(gql_graphiql),
          )
  })
  .bind("127.0.0.1:8010")?
  .run()
  .await
}
