use axum::{routing::get, Router};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(|| async { "UPSTREAM OK" }));
    let addr: SocketAddr = "0.0.0.0:3000".parse().unwrap();
    axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap();
}