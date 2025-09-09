//! Integration tests for the HTTP module.
//!
//! These tests use a stub PDP service over a Unix domain socket and a tiny in-process upstream to
//! verify that the HTTP reverse proxy calls the PDP, injects headers, and forwards requests.

use appgate_ipc::pdp::{
    p_d_p_server::{Pdp, PdpServer},
    DecisionRequest, DecisionResponse,
};
use appgate_ipc::uds_server;
use axum::{routing::get, Router};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tokio::{task, time::sleep};
use tonic::{Request as TRequest, Response as TResponse, Status};

/// Stub PDP server that always allows and injects a demo subject header.
struct AllowAll;

#[tonic::async_trait]
impl Pdp for AllowAll {
    async fn decide(
        &self,
        _req: TRequest<DecisionRequest>,
    ) -> Result<TResponse<DecisionResponse>, Status> {
        let mut inject = HashMap::new();
        inject.insert("X-User-Sub".to_string(), "demo".to_string());
        Ok(TResponse::new(DecisionResponse {
            allow: true,
            expiry: "2099-01-01T00:00:00Z".to_string(),
            claims: HashMap::new(),
            inject,
            reason: "allow".into(),
        }))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proxies_when_pdp_allows() {
    // 1) Start stub PDP over UDS
    let uds = "/tmp/appgate-test-pdp.sock";
    // remove existing socket if present
    let _ = std::fs::remove_file(uds);
    let svc = PdpServer::new(AllowAll);
    task::spawn(async move {
        uds_server(svc, uds).await.unwrap();
    });

    // 2) Start tiny upstream (returns "OK")
    async fn ok() -> &'static str {
        "OK"
    }
    let upstream = Router::new().route("/", get(ok));
    let up_addr: SocketAddr = "127.0.0.1:38080".parse().unwrap();
    task::spawn(async move {
        axum::Server::bind(&up_addr)
            .serve(upstream.into_make_service())
            .await
            .unwrap();
    });

    // 3) Spawn the HTTP module binary as a child process
    let http_bin = env!("CARGO_BIN_EXE_appgate-mod-http");
    // note: spawn instead of status() so we don't block; we'll kill it later
    let mut child = std::process::Command::new(http_bin)
        .args(&[
            "--bind",
            "127.0.0.1:38081",
            "--pdp-uds",
            uds,
            "--upstream",
            "http://127.0.0.1:38080",
        ])
        .spawn()
        .expect("spawn http module");

    // give processes time to start
    sleep(Duration::from_millis(500)).await;

    // 4) Send a request to the HTTP module and expect success
    let client = hyper::Client::new();
    let req = hyper::Request::builder()
        .uri("http://127.0.0.1:38081/")
        .body(hyper::Body::empty())
        .unwrap();
    let resp = client.request(req).await.expect("send request");
    assert!(
        resp.status().is_success(),
        "unexpected status: {}",
        resp.status()
    );
    let bytes = hyper::body::to_bytes(resp.into_body()).await.unwrap();
    assert_eq!(&bytes[..], b"OK");

    // terminate the HTTP module process
    let _ = child.kill();
}
