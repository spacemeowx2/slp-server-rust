mod graphql;
mod slp;
mod graphql_ws_filter;

use graphql::{schema, Context};
use slp::UDPServer;
use std::net::SocketAddr;
use warp::Filter;
use serde::Serialize;
use std::convert::Infallible;
use graphql_ws_filter::make_graphql_ws_filter;
use warp::filters::BoxedFilter;
use env_logger::Env;

#[derive(Serialize)]
struct Info {
    online: i32,
    version: String,
}

async fn server_info(context: Context) -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::json(&context.udp_server.server_info().await))
}

fn make_state(udp_server: &UDPServer) -> BoxedFilter<(Context,)> {
    let udp_server = udp_server.clone();
    warp::any().map(move || Context { udp_server: udp_server.clone() }).boxed()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::from_env(Env::default().default_filter_or("slp_server_rust=info")).init();

    let port: u16 = 11451;
    let bind_address = format!("{}:{}", "0.0.0.0", port);
    let udp_server = UDPServer::new(&bind_address).await?;

    log::info!("Listening on {}", bind_address);

    let graphql_filter = juniper_warp::make_graphql_filter(schema(), make_state(&udp_server));
    // let graphql_ws_filter = make_graphql_ws_filter(schema(), make_state(&udp_server));

    let socket_addr: &SocketAddr = &bind_address.parse().unwrap();

    let log = warp::log("warp_server");
    let routes = (
        warp::path("info")
            .and(make_state(&udp_server))
            .and_then(server_info)
        .or(warp::post()
            .and(graphql_filter))
        // TODO: enable ws until https://github.com/graphql-rust/juniper/issues/589 is fixed
        // .or(
        //     warp::get()
        //     .and(graphql_ws_filter))
    )
    .or(warp::get()
        .and(juniper_warp::playground_filter("/", Some("/"))))
        .with(log);

    warp::serve(routes)
        .run(*socket_addr)
        .await;

    Ok(())
}
