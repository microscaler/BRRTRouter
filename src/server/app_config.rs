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
    /// Optional HTTP rate limiting (Epic 13.2). Absent / disabled → no-op.
    pub rate_limit: Option<RateLimitYamlConfig>,
}

/// YAML shape for [`crate::middleware::RateLimitMiddleware`] (Epic 13.2).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RateLimitYamlConfig {
    /// When false or absent with no other fields forcing enable, middleware is skipped.
    pub enabled: Option<bool>,
    /// Max requests per window (global default).
    pub requests: Option<u64>,
    /// Window length in seconds (default 60).
    pub window_secs: Option<u64>,
    /// Soft cap on distinct client keys (default 10000).
    pub max_keys: Option<usize>,
    /// `subject_then_ip` (default) | `subject` | `ip`
    pub key: Option<String>,
    /// Per-handler overrides (handler_name → max requests / window).
    pub routes: Option<HashMap<String, u64>>,
    /// When true, OPTIONS does not consume tokens.
    pub skip_options: Option<bool>,
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

impl RateLimitYamlConfig {
    /// Convert YAML config into a middleware config. Returns `None` when disabled / unset.
    pub fn to_middleware_config(&self) -> Option<crate::middleware::RateLimitConfig> {
        let enabled = self.enabled.unwrap_or(false);
        if !enabled {
            return None;
        }
        use crate::middleware::{RateLimitConfig, RateLimitKeyMode};
        let key_mode = match self.key.as_deref().unwrap_or("subject_then_ip") {
            "ip" => RateLimitKeyMode::Ip,
            "subject" => RateLimitKeyMode::Subject,
            _ => RateLimitKeyMode::SubjectThenIp,
        };
        Some(RateLimitConfig {
            enabled: true,
            requests: self.requests.unwrap_or(100).max(1),
            window: std::time::Duration::from_secs(self.window_secs.unwrap_or(60).max(1)),
            max_keys: self.max_keys.unwrap_or(10_000),
            key_mode,
            route_limits: self.routes.clone().unwrap_or_default(),
            skip_options: self.skip_options.unwrap_or(false),
        })
    }
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
