use appgate_ctrl::Config;
use std::path::PathBuf;

#[test]
fn config_ok() {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../config/appgate.toml");
    let txt = std::fs::read_to_string(p).expect("read config file");
    let cfg: Config = toml::from_str(&txt).expect("parse config");
    assert!(cfg.validate().is_ok());
}

#[test]
fn config_too_short_cookie() {
    let toml_str = r#"
        [global]
        run_dir = "/run/x"
        log_level = "info"

        [certs]
        trust_store = "/etc/ca.pem"

        [auth.oidc]
        issuer = "https://kc/realms/main"
        client_id = "x"
        client_secret = "env:K"
        redirect_uri = "https://app/oidc/callback"
        cookie_name = "abc"
        cookie_domain = "example.com"
        session_ttl_seconds = 3600
    "#;
    let cfg: Config = toml::from_str(toml_str).expect("parse inline config");
    assert!(cfg.validate().is_err());
}
