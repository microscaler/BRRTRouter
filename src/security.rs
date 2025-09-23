use crate::spec::SecurityScheme;
use std::collections::HashMap;
use std::time::{Duration, Instant};

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
        Self {
            signature: signature.into(),
            cookie_name: None,
        }
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
        scopes
            .iter()
            .all(|s| token_scopes.split_whitespace().any(|ts| ts == s))
    }
}

impl SecurityProvider for BearerJwtProvider {
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
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
        Self {
            signature: signature.into(),
            cookie_name: None,
        }
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
            SecurityScheme::OAuth2 { .. } => {}
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

/// JWKS-based Bearer provider for production integrations.
/// Fetches keys from a JWKS URL and validates JWTs (signature and claims).
pub struct JwksBearerProvider {
    jwks_url: String,
    iss: Option<String>,
    aud: Option<String>,
    leeway_secs: u64,
    cache_ttl: Duration,
    // kid -> DecodingKey
    cache: std::sync::Mutex<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>,
}

impl JwksBearerProvider {
    pub fn new(jwks_url: impl Into<String>) -> Self {
        Self {
            jwks_url: jwks_url.into(),
            iss: None,
            aud: None,
            leeway_secs: 30,
            cache_ttl: Duration::from_secs(300),
            cache: std::sync::Mutex::new((
                Instant::now() - Duration::from_secs(1000),
                HashMap::new(),
            )),
        }
    }

    pub fn issuer(mut self, iss: impl Into<String>) -> Self {
        self.iss = Some(iss.into());
        self
    }
    pub fn audience(mut self, aud: impl Into<String>) -> Self {
        self.aud = Some(aud.into());
        self
    }
    pub fn leeway(mut self, secs: u64) -> Self {
        self.leeway_secs = secs;
        self
    }
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    fn extract_token<'a>(&self, req: &'a SecurityRequest) -> Option<&'a str> {
        req.headers
            .get("authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
    }

    fn refresh_jwks_if_needed(&self) {
        let mut guard = self.cache.lock().unwrap();
        let (last, map) = &mut *guard;
        if last.elapsed() < self.cache_ttl && !map.is_empty() {
            return;
        }
        drop(guard);
        // Fetch outside lock
        let resp = match reqwest::blocking::get(&self.jwks_url) {
            Ok(r) => r,
            Err(_) => return,
        };
        let body = match resp.text() {
            Ok(t) => t,
            Err(_) => return,
        };
        let parsed: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => return,
        };
        let mut new_map: HashMap<String, jsonwebtoken::DecodingKey> = HashMap::new();
        if let Some(keys) = parsed.get("keys").and_then(|v| v.as_array()) {
            for k in keys {
                let kid = k.get("kid").and_then(|v| v.as_str()).unwrap_or("");
                let kty = k.get("kty").and_then(|v| v.as_str()).unwrap_or("");
                let alg = k.get("alg").and_then(|v| v.as_str()).unwrap_or("");
                // HMAC (oct) keys for HS* algorithms
                if kty.eq_ignore_ascii_case("oct")
                    && (alg.eq_ignore_ascii_case("HS256")
                        || alg.eq_ignore_ascii_case("HS384")
                        || alg.eq_ignore_ascii_case("HS512"))
                {
                    if let Some(kval) = k.get("k").and_then(|v| v.as_str()) {
                        // base64url decode secret
                        if let Ok(secret) =
                            base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(kval)
                        {
                            let dk = jsonwebtoken::DecodingKey::from_secret(&secret);
                            new_map.insert(kid.to_string(), dk);
                        }
                    }
                    continue;
                }
                // RSA public keys for RS* algorithms
                if kty.eq_ignore_ascii_case("RSA")
                    && (alg.eq_ignore_ascii_case("RS256")
                        || alg.eq_ignore_ascii_case("RS384")
                        || alg.eq_ignore_ascii_case("RS512"))
                {
                    let n = match k.get("n").and_then(|v| v.as_str()) {
                        Some(v) => v,
                        None => continue,
                    };
                    let e = match k.get("e").and_then(|v| v.as_str()) {
                        Some(v) => v,
                        None => continue,
                    };
                    // jsonwebtoken expects base64url-encoded components for RSA
                    if let Ok(dk) = jsonwebtoken::DecodingKey::from_rsa_components(n, e) {
                        new_map.insert(kid.to_string(), dk);
                    }
                    continue;
                }
                // Unsupported kty/alg combinations are skipped
            }
        }
        let mut guard = self.cache.lock().unwrap();
        *guard = (Instant::now(), new_map);
    }

    fn get_key_for(&self, kid: &str) -> Option<jsonwebtoken::DecodingKey> {
        self.refresh_jwks_if_needed();
        let guard = self.cache.lock().unwrap();
        guard.1.get(kid).cloned()
    }
}

impl SecurityProvider for JwksBearerProvider {
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
            _ => return false,
        }
        let token = match self.extract_token(req) {
            Some(t) => t,
            None => return false,
        };
        // Parse header to locate kid/alg
        let header = match jsonwebtoken::decode_header(token) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let kid = match header.kid {
            Some(k) => k,
            None => return false,
        };
        let key = match self.get_key_for(&kid) {
            Some(k) => k,
            None => return false,
        };
        let selected_alg = match header.alg {
            jsonwebtoken::Algorithm::HS256 => jsonwebtoken::Algorithm::HS256,
            jsonwebtoken::Algorithm::HS384 => jsonwebtoken::Algorithm::HS384,
            jsonwebtoken::Algorithm::HS512 => jsonwebtoken::Algorithm::HS512,
            jsonwebtoken::Algorithm::RS256 => jsonwebtoken::Algorithm::RS256,
            jsonwebtoken::Algorithm::RS384 => jsonwebtoken::Algorithm::RS384,
            jsonwebtoken::Algorithm::RS512 => jsonwebtoken::Algorithm::RS512,
            _ => return false,
        };
        let mut validation = jsonwebtoken::Validation::new(selected_alg);
        validation.validate_exp = true;
        validation.set_required_spec_claims(&["exp"]);
        validation.leeway = self.leeway_secs;
        if let Some(ref iss) = self.iss {
            validation.set_issuer(&[iss]);
        }
        if let Some(ref aud) = self.aud {
            validation.set_audience(&[aud]);
        }
        let data: Result<jsonwebtoken::TokenData<serde_json::Value>, _> =
            jsonwebtoken::decode(token, &key, &validation);
        let claims = match data {
            Ok(d) => d.claims,
            Err(_) => return false,
        };
        // scope check
        let token_scopes = claims.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        scopes
            .iter()
            .all(|s| token_scopes.split_whitespace().any(|ts| ts == s))
    }
}

/// Remote API key verification provider with simple caching.
pub struct RemoteApiKeyProvider {
    verify_url: String,
    timeout_ms: u64,
    cache_ttl: Duration,
    cache: std::sync::Mutex<HashMap<String, (Instant, bool)>>,
    header_name: String,
}

impl RemoteApiKeyProvider {
    pub fn new(verify_url: impl Into<String>) -> Self {
        Self {
            verify_url: verify_url.into(),
            timeout_ms: 500,
            cache_ttl: Duration::from_secs(60),
            cache: std::sync::Mutex::new(HashMap::new()),
            header_name: "x-api-key".to_string(),
        }
    }
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }
    pub fn header_name(mut self, name: impl Into<String>) -> Self {
        self.header_name = name.into().to_ascii_lowercase();
        self
    }

    fn extract_key<'a>(&self, req: &'a SecurityRequest, header_name: &str) -> Option<&'a str> {
        // Prefer named header, also accept Authorization: Bearer <key>
        req.headers
            .get(header_name)
            .map(|s| s.as_str())
            .or_else(|| {
                req.headers
                    .get("authorization")
                    .and_then(|h| h.strip_prefix("Bearer "))
            })
    }
}

impl SecurityProvider for RemoteApiKeyProvider {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        let (name, location) = match scheme {
            SecurityScheme::ApiKey { name, location, .. } if location == "header" => {
                (name.to_ascii_lowercase(), location.as_str())
            }
            _ => return false,
        };
        let key = match self
            .extract_key(req, &self.header_name)
            .or_else(|| self.extract_key(req, &name))
        {
            Some(k) => k,
            None => return false,
        };
        // Cache lookup
        if let Some((ts, ok)) = self.cache.lock().unwrap().get(key).cloned() {
            if ts.elapsed() < self.cache_ttl {
                return ok;
            }
        }
        // Remote verify
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .build();
        let ok = match client {
            Ok(c) => match c.get(&self.verify_url).header("X-API-Key", key).send() {
                Ok(r) => r.status().is_success(),
                Err(_) => false,
            },
            Err(_) => false,
        };
        self.cache
            .lock()
            .unwrap()
            .insert(key.to_string(), (Instant::now(), ok));
        ok
    }
}
