pub mod pdp {
    tonic::include_proto!("appgate.pdp");
}

use anyhow::Result;
use tonic::transport::server::Router;
use tonic::transport::{Endpoint, Server};
use tower::service_fn;

use std::{path::Path, sync::Arc};
use tokio::net::UnixListener;

pub async fn uds_server<S>(svc: S, uds_path: &str) -> Result<()>
where
    S: tonic::codegen::Service<
            http::Request<hyper::body::Incoming>,
            Response = http::Response<tonic::body::BoxBody>,
        > + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    if Path::new(uds_path).exists() {
        tokio::fs::remove_file(uds_path).await.ok();
    }
    let uds = UnixListener::bind(uds_path)?;
    let incoming = tokio_stream::wrappers::UnixListenerStream::new(uds);
    Server::builder()
        .add_service(svc)
        .serve_with_incoming(incoming)
        .await?;
    Ok(())
}

// Client connector for UDS: use http+unix “h2c over UDS”
pub async fn uds_channel(uds_path: &str) -> Result<tonic::transport::Channel> {
    let path = format!("http://localhost"); // dummy
    let ep = Endpoint::try_from(path)?.connect_with_connector(service_fn(move |_| {
        let p = uds_path.to_owned();
        async move { tokio::net::UnixStream::connect(p).await }
    }));
    Ok(ep.await?)
}
