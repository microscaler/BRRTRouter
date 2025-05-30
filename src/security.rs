use crate::spec::SecurityScheme;
use std::collections::HashMap;

pub struct SecurityRequest<'a> {
    pub headers: &'a HashMap<String, String>,
    pub query: &'a HashMap<String, String>,
    pub cookies: &'a HashMap<String, String>,
}

pub trait SecurityProvider: Send + Sync {
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool;
}

use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;

/// Simple Bearer/JWT provider that validates tokens embedded in the
/// `Authorization` header or a cookie.
///
/// Tokens are expected to have the form `header.payload.signature` where the
/// signature part must match the configured `signature` string. Only the
/// payload section is inspected for a whitespace separated `scope` field.
pub struct BearerJwtProvider {
    signature: String,
    cookie_name: Option<String>,
}

impl BearerJwtProvider {
    pub fn new(signature: impl Into<String>) -> Self {
        Self { signature: signature.into(), cookie_name: None }
    }

    /// Configure the cookie name used to read the token.
    pub fn cookie_name(mut self, name: impl Into<String>) -> Self {
        self.cookie_name = Some(name.into());
        self
    }

    fn extract_token<'a>(&self, req: &'a SecurityRequest) -> Option<&'a str> {
        if let Some(name) = &self.cookie_name {
            if let Some(t) = req.cookies.get(name) {
                return Some(t);
            }
        }
        req.headers
            .get("authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
    }

    fn validate_token(&self, token: &str, scopes: &[String]) -> bool {
        let mut parts = token.split('.');
        let header = parts.next();
        let payload = parts.next();
        let sig = parts.next();
        if header.is_none() || payload.is_none() || sig != Some(self.signature.as_str()) {
            return false;
        }
        let payload_bytes = match general_purpose::STANDARD.decode(payload.unwrap()) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let json: Value = match serde_json::from_slice(&payload_bytes) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let token_scopes = json.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        scopes.iter().all(|s| token_scopes.split_whitespace().any(|ts| ts == s))
    }
}

impl SecurityProvider for BearerJwtProvider {
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {},
            _ => return false,
        }
        let token = match self.extract_token(req) {
            Some(t) => t,
            None => return false,
        };
        self.validate_token(token, scopes)
    }
}

/// OAuth2 provider using the same simple JWT validation as `BearerJwtProvider`.
pub struct OAuth2Provider {
    signature: String,
    cookie_name: Option<String>,
}

impl OAuth2Provider {
    pub fn new(signature: impl Into<String>) -> Self {
        Self { signature: signature.into(), cookie_name: None }
    }

    pub fn cookie_name(mut self, name: impl Into<String>) -> Self {
        self.cookie_name = Some(name.into());
        self
    }

    fn extract_token<'a>(&self, req: &'a SecurityRequest) -> Option<&'a str> {
        if let Some(name) = &self.cookie_name {
            if let Some(t) = req.cookies.get(name) {
                return Some(t);
            }
        }
        req.headers
            .get("authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
    }
}

impl SecurityProvider for OAuth2Provider {
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::OAuth2 { .. } => {},
            _ => return false,
        }
        let token = match self.extract_token(req) {
            Some(t) => t,
            None => return false,
        };
        // Reuse BearerJwtProvider logic
        let helper = BearerJwtProvider {
            signature: self.signature.clone(),
            cookie_name: None,
        };
        helper.validate_token(token, scopes)
    }
}

