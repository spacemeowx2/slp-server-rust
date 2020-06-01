#[macro_use]
extern crate lazy_static;

mod graphql;
mod slp;
mod graphql_ws_filter;
mod util;
mod plugin;
#[cfg(test)]
mod test;
mod panic;

use graphql::{schema, Context};
use slp::UDPServerBuilder;
use std::net::SocketAddr;
use serde::Serialize;
use std::convert::Infallible;
use graphql_ws_filter::make_graphql_ws_filter;
use warp::{Filter, filters::BoxedFilter, http::Method};
use env_logger::Env;
use structopt::StructOpt;

macro_rules! version_string {
    () => ( concat!(std::env!("CARGO_PKG_VERSION"), "-", std::env!("GIT_HASH")) )
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "slp-server-rust",
    version = version_string!(),
    author = "imspace <spacemeowx2@gmail.com>",
    about = "switch-lan-play Server written in Rust",
)]
struct Opt {
    /// Sets server listening port
    #[structopt(
        short,
        long,
        default_value = "11451",
    )]
    port: u16,
    /// Token for admin query. If not preset, no one can query admin information.
    #[structopt(long)]
    admin_token: Option<String>,
    /// Don't send broadcast to idle clients
    #[structopt(short, long)]
    ignore_idle: bool,
}

#[derive(Serialize)]
struct Info {
    online: i32,
    version: String,
}

async fn server_info(context: Context) -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::json(&context.udp_server.server_info().await))
}

fn make_state(context: &Context) -> BoxedFilter<(Context,)> {
    let ctx = context.clone();
    warp::any().map(move || ctx.clone()).boxed()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::from_env(Env::default().default_filter_or("slp_server_rust=info")).init();
    panic::set_panic_hook();
    let opt = Opt::from_args();

    if opt.ignore_idle {
        log::info!("--ignore-idle is not tested, bugs are expected");
    }

    let bind_address = format!("{}:{}", "0.0.0.0", opt.port);
    let socket_addr: &SocketAddr = &bind_address.parse().unwrap();

    let udp_server = UDPServerBuilder::new()
        .ignore_idle(opt.ignore_idle)
        .build(socket_addr)
        .await?;
    plugin::register_plugins(&udp_server).await;

    let context = Context::new(udp_server, opt.admin_token);

    log::info!("Listening on {}", bind_address);

    let graphql_filter = juniper_warp::make_graphql_filter(schema(), make_state(&context));
    let graphql_ws_filter = make_graphql_ws_filter(schema(), make_state(&context));

    let cors = warp::cors()
        .allow_headers(vec!["content-type", "x-apollo-tracing"])
        .allow_methods(&[Method::POST])
        .allow_any_origin();

    let log = warp::log("warp_server");
    let routes = (
        warp::path("info")
            .and(make_state(&context))
            .and_then(server_info)
        .or(warp::post()
            .and(graphql_filter))
        .or(
            warp::get()
            .and(graphql_ws_filter))
    )
    .or(warp::get()
        .and(juniper_warp::playground_filter("/", Some("/"))))
        .with(log)
        .with(cors);

    warp::serve(routes)
        .run(*socket_addr)
        .await;

    Ok(())
}
