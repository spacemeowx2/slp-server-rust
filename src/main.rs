#[macro_use]
extern crate lazy_static;

mod graphql;
mod panic;
mod plugin;
mod slp;
#[cfg(test)]
mod test;
mod util;

use async_graphql::http::{playground_source, GraphQLPlaygroundConfig, ALL_WEBSOCKET_PROTOCOLS};
use async_graphql_axum::{GraphQLProtocol, GraphQLWebSocket};
use axum::{
    body::Body,
    extract::WebSocketUpgrade,
    response::{Html, IntoResponse, Response},
    routing::{get, post_service},
    Extension, Json, Router,
};
use env_logger::Env;
use graphql::{schema, Ctx, SlpServerSchema};
use serde::Serialize;
use slp::{ServerInfo, UDPServerBuilder};
use std::net::SocketAddr;
use structopt::StructOpt;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

macro_rules! version_string {
    () => {
        concat!(std::env!("CARGO_PKG_VERSION"), "-", std::env!("GIT_HASH"))
    };
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
    #[structopt(short, long, default_value = "11451")]
    port: u16,
    /// Token for admin query. If not preset, no one can query admin information.
    #[structopt(long)]
    admin_token: Option<String>,
    /// Don't send broadcast to idle clients
    #[structopt(short, long)]
    ignore_idle: bool,
    /// Block rules
    #[structopt(short, long, default_value = "tcp:5000,tcp:21", use_delimiter = true)]
    block_rules: Vec<plugin::blocker::Rule>,
}

#[derive(Serialize)]
struct Info {
    online: i32,
    version: String,
}

async fn server_info(Extension(context): Extension<Ctx>) -> Json<ServerInfo> {
    Json(context.udp_server.server_info().await)
}

async fn index_get(
    Extension(executor): Extension<SlpServerSchema>,
    protocol: Option<GraphQLProtocol>,
    upgrade: Option<WebSocketUpgrade>,
) -> Response<Body> {
    if let (Some(protocol), Some(upgrade)) = (protocol, upgrade) {
        let executor = executor.clone();

        let resp = upgrade
            .protocols(ALL_WEBSOCKET_PROTOCOLS)
            .on_upgrade(move |stream| GraphQLWebSocket::new(stream, executor, protocol).serve());

        resp.into_response()
    } else {
        Html(playground_source(
            GraphQLPlaygroundConfig::new("/").subscription_endpoint("/"),
        ))
        .into_response()
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("slp_server_rust=info")).init();
    panic::set_panic_hook();

    tokio::spawn(async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to receive ctrl-c");
        log::info!("Exiting by ctrl-c");
        std::process::exit(0);
    });
    #[cfg(unix)]
    tokio::spawn(async {
        use tokio::signal::unix;
        unix::signal(unix::SignalKind::terminate())
            .expect("Failed to receive SIGTERM")
            .recv()
            .await;
        log::info!("Exiting by SIGTERM");
        std::process::exit(0);
    });

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
    if opt.block_rules.len() > 0 {
        log::info!("Applying {} rules", opt.block_rules.len());
        log::debug!("rules: {:?}", opt.block_rules);
        udp_server
            .get_plugin(&plugin::blocker::BLOCKER_TYPE, |b| {
                b.map(|b| b.set_block_rules(opt.block_rules.clone()))
            })
            .await;
    }

    let context = Ctx::new(udp_server, opt.admin_token);

    log::info!("Listening on {}", bind_address);

    let executor = schema(&context);
    let graphql_service = async_graphql_axum::GraphQL::new(executor.clone());

    let app = Router::new()
        .route("/", post_service(graphql_service).get(index_get))
        .route("/info", get(server_info))
        .layer(Extension(executor))
        .layer(Extension(context))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_headers([
                    http::header::CONTENT_TYPE,
                    http::header::HeaderName::from_static("x-apollo-tracing"),
                ])
                .allow_methods([http::Method::POST])
                .allow_origin(Any),
        );

    let listener = tokio::net::TcpListener::bind(socket_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
