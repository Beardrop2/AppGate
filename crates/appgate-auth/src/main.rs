use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;
use tonic::{Request, Response, Status};
use appgate_ipc::{pdp::{p_d_p_server::{Pdp, PdpServer}, DecisionRequest, DecisionResponse}, uds_server};
use std::sync::Arc;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value="/etc/appgate/appgate.toml")]
    config: String,
    #[arg(long, default_value="/run/appgate/pdp.sock")]
    uds: String,
    #[arg(long, default_value="config/policy/foundry.toml")]
    policy: String,
}

struct PdpSvc {
    policy: Arc<appgate_policy::Policy>,
}

#[tonic::async_trait]
impl Pdp for PdpSvc {
    async fn decide(&self, req: Request<DecisionRequest>) -> Result<Response<DecisionResponse>, Status> {
        let r = req.into_inner();
        // TODO: validate session_token (OIDC/session cookie verification).
        // For MVP, treat token presence as authenticated and fake groups:
        let groups = vec!["foundry-players".to_string(), "foundry-admin".to_string()];
        let (allow, inject, reason) = self.policy.decide(&r.protocol, &r.resource, &groups);
        let resp = DecisionResponse {
            allow,
            expiry: chrono::Utc::now().checked_add_signed(chrono::Duration::minutes(30)).unwrap().to_rfc3339(),
            claims: [("sub".to_string(), "demo-sub".to_string())].into_iter().collect(),
            inject: inject.unwrap_or_default(),
            reason,
        };
        Ok(Response::new(resp))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    let args = Args::parse();
    let policy = Arc::new(appgate_policy::Policy::load(&args.policy)?);

    let svc = PdpServer::new(PdpSvc { policy });
    tracing::info!("PDP listening on {}", args.uds);
    uds_server(svc, &args.uds).await?;
    Ok(())
}