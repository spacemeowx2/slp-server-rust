mod graphql;
mod slp;

use async_std::sync::{Arc, RwLock};
use async_std::task;
use graphql::{schema, Context};
use slp::UDPServer;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use warp::{http::Response, Filter};
use juniper_warp::subscriptions::graphql_subscriptions;
use juniper_subscriptions::Coordinator;
use std::{pin::Pin, time::Duration};
use futures::{Future, FutureExt as _, Stream};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let port: u16 = 11451;
    let bind_address = format!("{}:{}", "0.0.0.0", port);
    let udp_server = UDPServer::new();
    let s1 = udp_server.clone();
    let s2 = udp_server.clone();

    log::info!("Listening on {}", bind_address);

    let state = warp::any().map(move || Context { udp_server: s1.clone() });
    let graphql_filter = juniper_warp::make_graphql_filter(schema(), state.boxed());
    let socket_addr: &SocketAddr = &bind_address.parse().unwrap();

    let sub_state = warp::any().map(move || Context { udp_server: s2.clone() });
    let coordinator = Arc::new(juniper_subscriptions::Coordinator::new(schema()));

    let log = warp::log("warp_server");
    let routes = (warp::get()
        .and(warp::ws())
        .and(sub_state.clone())
        .and(warp::any().map(move || Arc::clone(&coordinator)))
        .map(
            |ws: warp::ws::Ws,
             ctx: Context,
             coordinator: Arc<Coordinator<'static, _, _, _, _, _>>| {
                ws.on_upgrade(|websocket| -> Pin<Box<dyn Future<Output = ()> + Send>> {
                    graphql_subscriptions(websocket, coordinator, ctx).boxed()
                })
            },
        ))
    .or(warp::get()
        .and(juniper_warp::playground_filter("/", Some("/"))))
    .or(warp::post()
        .and(graphql_filter))
    .with(log);

    warp::serve(routes)
        .run(*socket_addr)
        .await;

    Ok(())
}
