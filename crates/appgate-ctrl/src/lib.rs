use serde::Deserialize;
use thiserror::Error;

/// Errors that can occur during configuration validation
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required configuration key was missing
    #[error("missing key: {0}")]
    Missing(&'static str),
    /// A configuration value was invalid
    #[error("invalid value for {key}: {reason}")]
    Invalid { key: &'static str, reason: String },
}

/// Global configuration values
#[derive(Debug, Deserialize)]
pub struct Global {
    pub run_dir: String,
    pub log_level: String,
}

/// Certificate configuration values
#[derive(Debug, Deserialize)]
pub struct Certs {
    pub trust_store: String,
}

/// OIDC configuration values
#[derive(Debug, Deserialize)]
pub struct Oidc {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub cookie_name: String,
    pub cookie_domain: String,
    pub session_ttl_seconds: u64,
}

/// Top-level configuration structure
#[derive(Debug, Deserialize)]
pub struct Config {
    pub global: Global,
    pub certs: Certs,
    #[serde(rename = "auth.oidc")]
    pub auth_oidc: Oidc,
}

impl Config {
    /// Validate configuration values for presence and basic constraints
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.global.run_dir.is_empty() {
            return Err(ConfigError::Missing("global.run_dir"));
        }
        if self.certs.trust_store.is_empty() {
            return Err(ConfigError::Missing("certs.trust_store"));
        }
        if self.auth_oidc.session_ttl_seconds < 60 {
            return Err(ConfigError::Invalid {
                key: "auth.oidc.session_ttl_seconds",
                reason: "must be >= 60".into(),
            });
        }
        if self.auth_oidc.cookie_name.len() < 5 {
            return Err(ConfigError::Invalid {
                key: "auth.oidc.cookie_name",
                reason: "too short".into(),
            });
        }
        Ok(())
    }
}
