use anyhow::Result;
use axum::{routing::get, Router};
use clap::Parser;
use prometheus::{Encoder, Registry, TextEncoder};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

use appgate_ctrl::Config;
use std::fs;
use toml;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "/etc/appgate/appgate.toml")]
    config: String,
    #[arg(long, default_value = "0.0.0.0:9100")]
    metrics_addr: String,
    #[arg(long, default_value = "0.0.0.0:9101")]
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
    // 1) logging
    init_json_logger();

    // 2) args
    let args = Args::parse();
    info!(?args, "appgate-ctrl starting");

    // 3) config load/validate
    let conf_text = fs::read_to_string(&args.config)?;
    let cfg: Config = toml::from_str(&conf_text)?;
    cfg.validate().map_err(|e| anyhow::anyhow!(e))?;

    // 4) health server (spawned)
    {
        let health_addr: SocketAddr = args.health_addr.parse()?;
        let health_app = Router::new().route("/healthz", get(|| async { "ok" }));
        let health_listener = TcpListener::bind(health_addr).await?;
        tokio::spawn(async move {
            if let Err(err) = axum::serve(health_listener, health_app).await {
                eprintln!("health server failed: {err}");
            }
        });
    }

    // 5) metrics server (main task)
    let metrics_addr: SocketAddr = args.metrics_addr.parse()?;
    let reg = Registry::new();
    let metrics_app = {
        // If you prefer, you can put Registry in axum state instead.
        let reg_clone = reg.clone();
        Router::new().route(
            "/metrics",
            get(move || {
                let reg = reg_clone.clone();
                async move {
                    let mf = reg.gather();
                    let mut buf = Vec::new();
                    TextEncoder::new().encode(&mf, &mut buf).unwrap();
                    String::from_utf8(buf).unwrap()
                }
            }),
        )
    };

    let metrics_listener = TcpListener::bind(metrics_addr).await?;
    axum::serve(metrics_listener, metrics_app).await?;
    Ok(())
}
