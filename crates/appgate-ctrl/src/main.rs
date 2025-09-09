use anyhow::Result;
use clap::Parser;
use axum::{routing::get, Router};
use prometheus::{Encoder, TextEncoder, Registry};
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value="/etc/appgate/appgate.toml")]
    config: String,
    #[arg(long, default_value="0.0.0.0:9100")]
    metrics_addr: String,
    #[arg(long, default_value="0.0.0.0:9101")]
    health_addr: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    let args = Args::parse();
    tracing::info!(?args, "appgate-ctrl starting");

    // TODO: load config, push keys to UDS, supervise children if desired.

    // health
    let health_app = Router::new().route("/healthz", get(|| async { "ok" }));
    tokio::spawn({
        let addr: SocketAddr = args.health_addr.parse().unwrap();
        async move { axum::Server::bind(&addr).serve(health_app.into_make_service()).await.unwrap(); }
    });

    // metrics
    let reg = Registry::new();
    let metrics_app = Router::new().route("/metrics", get(move || {
        let reg = reg.clone();
        async move {
            let mf = reg.gather();
            let mut buf = Vec::new();
            TextEncoder::new().encode(&mf, &mut buf).unwrap();
            String::from_utf8(buf).unwrap()
        }
    }));
    let addr: SocketAddr = args.metrics_addr.parse()?;
    axum::Server::bind(&addr).serve(metrics_app.into_make_service()).await?;
    Ok(())
}