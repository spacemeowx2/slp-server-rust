mod graphql;
mod slp;

use async_std::sync::{Arc, RwLock};
use async_std::task;
use graphql::{schema, Context};
use slp::UDPServer;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use warp::{http::Response, Filter};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let log = warp::log("warp_server");

    let port: u16 = 11451;
    let bind_address = format!("{}:{}", "0.0.0.0", port);

    let homepage = warp::path::end().map(|| {
        Response::builder()
            .header("content-type", "text/html")
            .body(format!(
                "<html><h1>juniper_warp</h1><div>visit <a href=\"/graphiql\">/graphiql</a></html>"
            ))
    });

    log::info!("Listening on {}", bind_address);

    let state = warp::any().map(move || Context {});
    let graphql_filter = juniper_warp::make_graphql_filter(schema(), state.boxed());
    let socket_addr: &SocketAddr = &bind_address.parse().unwrap();

    warp::serve(
        warp::get()
            .and(warp::path("graphiql"))
            .and(juniper_warp::graphiql_filter("/graphql"))
            .or(homepage)
            .or(warp::path("graphql").and(graphql_filter))
            .with(log),
    )
    .run(*socket_addr)
    .await;

    Ok(())
}
