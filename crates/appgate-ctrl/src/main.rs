use anyhow::Result;
use clap::Parser;
use axum::{routing::get, Router};
use prometheus::{Encoder, TextEncoder, Registry};
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;
use appgate_ctrl::Config;
use std::fs;
use toml;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value="/etc/appgate/appgate.toml")]
    config: String,
    #[arg(long, default_value="0.0.0.0:9100")]
    metrics_addr: String,
    #[arg(long, default_value="0.0.0.0:9101")]
    health_addr: String,
}

fn init_json_logger() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt()
        .with_env_filter(filter)
        .json()
        .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
        .with_current_span(true)
        .with_span_list(true)
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    // initialise structured JSON logger with RFC3339 timestamps
    init_json_logger();

    let args = Args::parse();
    tracing::info!(?args, "appgate-ctrl starting");

    // Load and validate configuration
    let conf_text = fs::read_to_string(&args.config)?;
    let cfg: Config = toml::from_str(&conf_text)?;
    cfg.validate().map_err(|e| anyhow::anyhow!(e))?;

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