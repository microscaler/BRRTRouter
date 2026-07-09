//! Shared YAML configuration types for generated and impl service binaries.
//!
//! Extracted from generated `main.rs` so Fix B (`run_app`) can load config once without
//! each service duplicating these structs.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// Top-level service configuration (`config/config.yaml`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct AppConfig {
    /// Server port (preferred over `PORT` env var).
    pub port: Option<u16>,
    pub security: Option<SecurityConfig>,
    pub http: Option<HttpConfig>,
    pub cors: Option<CorsConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SecurityConfig {
    pub api_keys: Option<HashMap<String, ApiKeyConfig>>,
    pub remote_api_keys: Option<HashMap<String, RemoteApiKeyConfig>>,
    pub bearer: Option<BearerConfig>,
    pub oauth2: Option<OAuth2Config>,
    pub jwks: Option<HashMap<String, JwksConfig>>,
    pub propelauth: Option<PropelAuthConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ApiKeyConfig {
    pub key: Option<String>,
    pub header_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteApiKeyConfig {
    pub verify_url: String,
    pub timeout_ms: Option<u64>,
    pub header_name: Option<String>,
    pub cache_ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct BearerConfig {
    pub signature: Option<String>,
    pub cookie_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct OAuth2Config {
    pub signature: Option<String>,
    pub cookie_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JwksConfig {
    pub jwks_url: String,
    pub iss: Option<String>,
    pub aud: Option<String>,
    pub leeway_secs: Option<u64>,
    pub cache_ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PropelAuthConfig {
    pub auth_url: String,
    pub audience: Option<String>,
    pub issuer: Option<String>,
    pub jwks_url: Option<String>,
    pub leeway_secs: Option<u64>,
    pub cache_ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct HttpConfig {
    pub keep_alive: Option<bool>,
    pub timeout_secs: Option<u64>,
    pub max_requests: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CorsConfig {
    pub origins: Option<Vec<String>>,
    pub allowed_headers: Option<Vec<String>>,
    pub allowed_methods: Option<Vec<String>>,
    pub allow_credentials: Option<bool>,
    pub expose_headers: Option<Vec<String>>,
    pub max_age: Option<u32>,
}

/// Load `config.yaml` using the same semantics as generated service mains.
pub fn load_app_config(path: &Path) -> io::Result<AppConfig> {
    match fs::read_to_string(path) {
        Ok(s) => serde_yaml::from_str::<AppConfig>(&s).map_err(|e| {
            io::Error::other(format!(
                "Invalid configuration file {}: {e}",
                path.display()
            ))
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            println!(
                "[config] {} not found; continuing with defaults",
                path.display()
            );
            Ok(AppConfig::default())
        }
        Err(e) => Err(io::Error::other(format!(
            "Failed to read configuration file {}: {e}",
            path.display()
        ))),
    }
}
