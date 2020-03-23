mod graphql;
mod slp;

use graphql::{schema, Context};
use slp::UDPServer;
use std::net::SocketAddr;
use warp::Filter;
use juniper_warp::subscriptions::graphql_subscriptions;
use juniper_subscriptions::Coordinator;
use std::pin::Pin;
use futures::{Future, FutureExt as _};
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let port: u16 = 11451;
    let bind_address = format!("{}:{}", "0.0.0.0", port);
    let udp_server = UDPServer::new(bind_address.clone());
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
        .map(|reply| {
            warp::reply::with_header(reply, "Sec-WebSocket-Protocol", "graphql-ws")
        })
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
