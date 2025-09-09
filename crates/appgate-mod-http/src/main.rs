use anyhow::Result;
use clap::Parser;
use axum::{Router, routing::any, extract::State};
use hyper::{Request, Response, Body, Client};
use tracing_subscriber::EnvFilter;
use appgate_ipc::{pdp::{p_d_p_client::PdpClient, DecisionRequest}, uds_channel};
use std::{net::SocketAddr, sync::Arc};

#[derive(Clone)]
struct AppState {
    pdp: Arc<tokio::sync::Mutex<PdpClient<tonic::transport::Channel>>>,
    upstream: http::Uri,
    cookie_name: String,
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value="0.0.0.0:8080")]
    bind: String,
    #[arg(long, default_value="/run/appgate/pdp.sock")]
    pdp_uds: String,
    #[arg(long, default_value="http://localhost:3000")]
    upstream: String,
    #[arg(long, default_value="appg_sess")]
    cookie_name: String,
}

async fn handler(State(st): State<AppState>, mut req: Request<Body>) -> Result<Response<Body>, axum::http::Error> {
    // Extract bearer/cookie as “session token” (MVP: raw cookie value)
    let token = req.headers().get(http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(';').find(|p| p.trim_start().starts_with(&st.cookie_name)))
        .and_then(|kv| kv.split_once('=')).map(|(_,v)| v.to_string())
        .unwrap_or_default();

    // PDP decision
    {
        let mut client = st.pdp.lock().await;
        let attrs = [("method","GET")].into_iter().map(|(k,v)|(k.to_string(),v.to_string())).collect();
        let dr = DecisionRequest {
            session_token: token,
            protocol: "http".into(),
            resource: req.uri().to_string(),
            peer: "unknown".into(),
            attributes: Some(appgate_ipc::pdp::Attributes{ kv: attrs }),
        };
        let resp = client.decide(dr).await.map_err(|_| axum::http::Error::new_infallible())?.into_inner();
        if !resp.allow {
            return Ok(Response::builder().status(403).body(Body::from("forbidden")).unwrap());
        }
        for (k,v) in resp.inject {
            req.headers_mut().insert(
                http::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                http::HeaderValue::from_str(&v).unwrap()
            );
        }
    }

    // Proxy to upstream
    let (mut parts, body) = req.into_parts();
    let mut uri = parts.uri.clone().into_parts();
    let upstream: http::Uri = st.upstream.clone();
    uri.scheme = upstream.scheme().cloned();
    uri.authority = upstream.authority().cloned();
    parts.uri = http::Uri::from_parts(uri).unwrap();
    let fwd_req = Request::from_parts(parts, body);
    let client = Client::new();
    match client.request(fwd_req).await {
        Ok(r) => Ok(r),
        Err(_) => Ok(Response::builder().status(502).body(Body::from("bad gateway")).unwrap())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    let args = Args::parse();
    let chan = uds_channel(&args.pdp_uds).await?;
    let pdp = Arc::new(tokio::sync::Mutex::new(appgate_ipc::pdp::p_d_p_client::PdpClient::new(chan)));

    let state = AppState {
        pdp,
        upstream: args.upstream.parse().unwrap(),
        cookie_name: args.cookie_name,
    };

    let app = Router::new().route("/*path", any(handler)).with_state(state);
    let addr: SocketAddr = args.bind.parse()?;
    tracing::info!("HTTP module on {}", addr);
    axum::Server::bind(&addr).serve(app.into_make_service()).await?;
    Ok(())
}