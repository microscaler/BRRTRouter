//! Security provider registration from `config.yaml` at startup.

use std::sync::Arc;

use crate::security::{JwksBearerProvider, RemoteApiKeyProvider};
use crate::spec::SecurityScheme;
use crate::{BearerJwtProvider, OAuth2Provider, SecurityProvider, SecurityRequest};

use super::app_config::AppConfig;
use super::service::AppService;

struct StaticApiKeyProvider {
    key: String,
    header_override: Option<String>,
}

impl SecurityProvider for StaticApiKeyProvider {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                "header" => {
                    let target = self.header_override.as_deref().unwrap_or(name);
                    req.get_header(&target.to_ascii_lowercase())
                        .map(|v| v == self.key)
                        .unwrap_or(false)
                }
                "query" => req.get_query(name).map(|v| v == self.key).unwrap_or(false),
                "cookie" => req.get_cookie(name).map(|v| v == self.key).unwrap_or(false),
                _ => false,
            },
            _ => false,
        }
    }
}

/// Register auth providers for each OpenAPI security scheme on the service.
pub fn register_security_from_config(
    service: &mut AppService,
    app_config: &AppConfig,
    test_api_key: Option<&str>,
) {
    let sec_cfg = app_config.security.as_ref();
    for (scheme_name, scheme) in service.security_schemes.clone() {
        match scheme {
            SecurityScheme::ApiKey { .. } => {
                register_api_key(service, sec_cfg, &scheme_name, test_api_key)
            }
            SecurityScheme::Http { ref scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {
                register_bearer(service, sec_cfg, &scheme_name);
            }
            SecurityScheme::OAuth2 { .. } => register_oauth2(service, sec_cfg, &scheme_name),
            _ => {}
        }
    }
}

fn register_api_key(
    service: &mut AppService,
    sec_cfg: Option<&super::app_config::SecurityConfig>,
    scheme_name: &str,
    test_api_key: Option<&str>,
) {
    let mut registered = false;
    if let Some(cfgs) = sec_cfg.and_then(|s| s.remote_api_keys.as_ref()) {
        if let Some(cfg) = cfgs.get(scheme_name) {
            let mut provider = RemoteApiKeyProvider::new(&cfg.verify_url);
            if let Some(ms) = cfg.timeout_ms {
                provider = provider.timeout_ms(ms);
            }
            if let Some(h) = cfg.header_name.as_ref() {
                provider = provider.header_name(h);
            }
            if let Some(ttl) = cfg.cache_ttl_secs {
                provider = provider.cache_ttl(std::time::Duration::from_secs(ttl));
            }
            println!(
                "[auth] register RemoteApiKeyProvider scheme={scheme_name} url={} header={} timeout_ms={:?} ttl_s={:?}",
                cfg.verify_url,
                cfg.header_name
                    .clone()
                    .unwrap_or_else(|| "(default X-API-Key)".into()),
                cfg.timeout_ms,
                cfg.cache_ttl_secs
            );
            service.register_security_provider(scheme_name, Arc::new(provider));
            registered = true;
        }
    }
    if !registered {
        if let Some(cfgs) = sec_cfg.and_then(|s| s.api_keys.as_ref()) {
            if let Some(cfg) = cfgs.get(scheme_name) {
                if let Some(key) = cfg.key.clone() {
                    println!(
                        "[auth] register StaticApiKeyProvider scheme={scheme_name} header_override={:?} key_len={}",
                        cfg.header_name,
                        key.len()
                    );
                    service.register_security_provider(
                        scheme_name,
                        Arc::new(StaticApiKeyProvider {
                            key,
                            header_override: cfg.header_name.clone(),
                        }),
                    );
                    registered = true;
                }
            }
        }
    }
    if !registered {
        let fallback = std::env::var("BRRTR_API_KEY")
            .ok()
            .or_else(|| test_api_key.map(str::to_string))
            .unwrap_or_else(|| "test123".to_string());
        println!(
            "[auth] register StaticApiKeyProvider scheme={scheme_name} from=fallback key_len={}",
            fallback.len()
        );
        service.register_security_provider(
            scheme_name,
            Arc::new(StaticApiKeyProvider {
                key: fallback,
                header_override: None,
            }),
        );
    }
}

fn register_bearer(
    service: &mut AppService,
    sec_cfg: Option<&super::app_config::SecurityConfig>,
    scheme_name: &str,
) {
    if register_jwks_from_propelauth(service, sec_cfg, scheme_name) {
        return;
    }
    if register_jwks_per_scheme(service, sec_cfg, scheme_name) {
        return;
    }
    let sig = sec_cfg
        .and_then(|s| s.bearer.as_ref())
        .and_then(|b| b.signature.clone())
        .or_else(|| std::env::var("BRRTR_BEARER_SIGNATURE").ok())
        .unwrap_or_else(|| "sig".into());
    let sig_len = sig.len();
    let mut p = BearerJwtProvider::new(sig);
    let cookie_opt = sec_cfg
        .and_then(|s| s.bearer.as_ref())
        .and_then(|b| b.cookie_name.clone());
    if let Some(cookie) = cookie_opt.clone() {
        p = p.cookie_name(cookie);
    }
    println!(
        "[auth] register BearerJwtProvider scheme={scheme_name} source=mock signature_len={sig_len} cookie={cookie_opt:?}"
    );
    service.register_security_provider(scheme_name, Arc::new(p));
}

fn register_oauth2(
    service: &mut AppService,
    sec_cfg: Option<&super::app_config::SecurityConfig>,
    scheme_name: &str,
) {
    if register_jwks_from_propelauth(service, sec_cfg, scheme_name) {
        return;
    }
    if register_jwks_per_scheme(service, sec_cfg, scheme_name) {
        return;
    }
    let sig = sec_cfg
        .and_then(|s| s.oauth2.as_ref())
        .and_then(|b| b.signature.clone())
        .or_else(|| std::env::var("BRRTR_OAUTH2_SIGNATURE").ok())
        .unwrap_or_else(|| "sig".into());
    let sig_len = sig.len();
    let mut p = OAuth2Provider::new(sig);
    let cookie_opt = sec_cfg
        .and_then(|s| s.oauth2.as_ref())
        .and_then(|b| b.cookie_name.clone());
    if let Some(cookie) = cookie_opt.clone() {
        p = p.cookie_name(cookie);
    }
    println!(
        "[auth] register OAuth2Provider scheme={scheme_name} source=mock signature_len={sig_len} cookie={cookie_opt:?}"
    );
    service.register_security_provider(scheme_name, Arc::new(p));
}

fn register_jwks_from_propelauth(
    service: &mut AppService,
    sec_cfg: Option<&super::app_config::SecurityConfig>,
    scheme_name: &str,
) -> bool {
    let Some(pa) = sec_cfg.and_then(|s| s.propelauth.as_ref()) else {
        return false;
    };
    let jwks_url = pa.jwks_url.clone().unwrap_or_else(|| {
        let base = pa.auth_url.trim_end_matches('/');
        format!("{base}/.well-known/jwks.json")
    });
    let mut p = JwksBearerProvider::new(&jwks_url);
    let issuer_opt: Option<&str> = pa.issuer.as_deref().or(Some(pa.auth_url.as_str()));
    if let Some(iss) = issuer_opt {
        p = p.issuer(iss);
    }
    if let Some(aud) = pa.audience.as_ref() {
        p = p.audience(aud.clone());
    }
    if let Some(leeway) = pa.leeway_secs {
        p = p.leeway(leeway);
    }
    if let Some(ttl) = pa.cache_ttl_secs {
        p = p.cache_ttl(std::time::Duration::from_secs(ttl));
    }
    println!(
        "[auth] register JwksBearerProvider scheme={scheme_name} source=propelauth jwks_url={jwks_url} iss={:?} aud={:?}",
        pa.issuer, pa.audience
    );
    service.register_security_provider(scheme_name, Arc::new(p));
    true
}

fn register_jwks_per_scheme(
    service: &mut AppService,
    sec_cfg: Option<&super::app_config::SecurityConfig>,
    scheme_name: &str,
) -> bool {
    let Some(jwks_map) = sec_cfg.and_then(|s| s.jwks.as_ref()) else {
        return false;
    };
    let Some(jwks) = jwks_map.get(scheme_name) else {
        return false;
    };
    let mut p = JwksBearerProvider::new(&jwks.jwks_url);
    if let Some(iss) = jwks.iss.as_deref() {
        p = p.issuer(iss);
    }
    if let Some(aud) = jwks.aud.as_deref() {
        p = p.audience(aud);
    }
    if let Some(leeway) = jwks.leeway_secs {
        p = p.leeway(leeway);
    }
    if let Some(ttl) = jwks.cache_ttl_secs {
        p = p.cache_ttl(std::time::Duration::from_secs(ttl));
    }
    println!(
        "[auth] register JwksBearerProvider scheme={scheme_name} source=per-scheme jwks_url={} iss={:?} aud={:?}",
        jwks.jwks_url, jwks.iss, jwks.aud
    );
    service.register_security_provider(scheme_name, Arc::new(p));
    true
}
