//! End-to-end authentication test using Keycloak and testcontainers.
//!
//! This test spins up a Keycloak container with a fixed realm, a tiny upstream service,
//! and the AppGate auth/PDP and HTTP module. It simulates a client performing an OIDC
//! Authorization Code + PKCE flow through the gateway and ultimately reaching the upstream.
//!
//! NOTE: The test is ignored by default until OIDC support is implemented in the gateway.

use testcontainers::{clients::Cli, images::generic::{GenericImage, WaitFor}, RunnableImage};
use once_cell::sync::Lazy;

static DOCKER: Lazy<Cli> = Lazy::new(|| Cli::default());

fn kc_image() -> RunnableImage<GenericImage> {
    RunnableImage::from(
        GenericImage::new("quay.io/keycloak/keycloak", "24.0.5")
            .with_env_var("KEYCLOAK_ADMIN", "admin")
            .with_env_var("KEYCLOAK_ADMIN_PASSWORD", "admin")
            .with_env_var("KC_HEALTH_ENABLED", "true")
            .with_volume((
                std::env::current_dir().unwrap().join("tests/fixtures/keycloak/realm-export.json").to_string_lossy().into_owned(),
                "/opt/keycloak/data/import/realm-export.json".into(),
            ))
            .with_wait_for(WaitFor::message_on_stderr("Running the server"))
            .with_exposed_port(8080)
            .with_cmd(vec!["start-dev", "--import-realm"]),
    )
}

#[test]
#[ignore]
fn e2e_oidc_login_and_reach_upstream() {
    let docker = &*DOCKER;
    let kc = docker.run(kc_image());
    let kc_port = kc.get_host_port_ipv4(8080);
    // Additional setup for tiny upstream and gateway would follow here.
    assert!(kc_port > 0);
}