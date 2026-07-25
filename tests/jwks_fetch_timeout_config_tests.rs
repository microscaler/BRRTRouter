//! The JWKS fetch timeout must come from configuration, not from a constant in the refresh path.
//!
//! `refresh_jwks_internal` used to build its `HttpFetchOptions` with a literal
//! `Duration::from_millis(200)`. That was sized for plaintext in-cluster
//! `http://…svc.cluster.local` JWKS URLs; it cannot cover DNS + TCP + a TLS handshake + chain
//! verification + the HTTP round trip to a hostname behind a TLS-terminating edge. With a literal
//! there was no deployment-time answer at all: every refresh timed out, no keyset was ever
//! cached, and every JWT was rejected.
//!
//! These tests drive a JWKS server that is *deliberately slower than 200ms* and prove that
//! whether the fetch succeeds is decided by configuration.
//!
//! The environment variable is process-global, so the cases here are serialised behind a mutex
//! and each restores whatever it found.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use brrtrouter::dispatcher::HeaderVec;
use brrtrouter::router::ParamVec;
use brrtrouter::security::{
    JwksBearerProvider, SecurityProvider, SecurityRequest, DEFAULT_JWKS_FETCH_TIMEOUT,
    JWKS_FETCH_TIMEOUT_ENV,
};
use brrtrouter::spec::SecurityScheme;
use jsonwebtoken::{Algorithm, EncodingKey, Header};

/// `set_var` is process-global; serialise every test that constructs a provider while it is set.
static ENV_GUARD: Mutex<()> = Mutex::new(());

/// Latency the JWKS server injects. Comfortably above the old hard-coded 200ms, comfortably below
/// the new default, so the two configurations give opposite outcomes against the same server.
const SERVER_LATENCY: Duration = Duration::from_millis(600);

fn bearer_scheme() -> SecurityScheme {
    SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: Some("JWT".to_string()),
        description: None,
    }
}

fn jwks(secret: &[u8], kid: &str) -> String {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);
    serde_json::json!({
        "keys": [{ "kty": "oct", "alg": "HS256", "kid": kid, "k": encoded }]
    })
    .to_string()
}

fn token(secret: &[u8], kid: &str) -> String {
    let header = Header {
        alg: Algorithm::HS256,
        kid: Some(kid.to_string()),
        typ: Some("at+jwt".to_string()),
        ..Header::default()
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = serde_json::json!({ "sub": "timeout-config", "exp": now + 300 });
    jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
}

fn validate(provider: &JwksBearerProvider, token: &str) -> bool {
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let query = ParamVec::new();
    let cookies = HeaderVec::new();
    let request = SecurityRequest {
        headers: &headers,
        query: &query,
        cookies: &cookies,
    };
    provider.validate(&bearer_scheme(), &[], &request)
}

/// A JWKS endpoint that always answers correctly, but only after `SERVER_LATENCY` —
/// the local stand-in for a TLS handshake to a real host.
fn start_slow_jwks_server(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0_u8; 2048];
            let _ = stream.read(&mut buf);
            thread::sleep(SERVER_LATENCY);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    format!("http://{address}/slow-issuer/.well-known/jwks.json")
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

struct EnvOverride {
    previous: Option<String>,
}

impl EnvOverride {
    fn set(value: &str) -> Self {
        let previous = std::env::var(JWKS_FETCH_TIMEOUT_ENV).ok();
        std::env::set_var(JWKS_FETCH_TIMEOUT_ENV, value);
        Self { previous }
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(JWKS_FETCH_TIMEOUT_ENV, value),
            None => std::env::remove_var(JWKS_FETCH_TIMEOUT_ENV),
        }
    }
}

/// Same server, two configurations, opposite outcomes: the timeout is genuinely configuration.
#[test]
fn refresh_timeout_is_taken_from_configuration() {
    let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());

    let secret = b"timeout-config-secret";
    let kid = "timeout-config-kid";
    let url = start_slow_jwks_server(jwks(secret, kid));

    // (a) Configured below the endpoint's latency: the refresh must fail. This is the case the
    //     old 200ms literal forced on everybody, and it is what "no keys, every token 401" looks
    //     like from the inside.
    {
        let _env = EnvOverride::set("150");
        let tight = JwksBearerProvider::new(&url);
        assert!(
            wait_until(Duration::from_secs(10), || tight
                .jwks_health()
                .fetch_failure
                > 0),
            "a 150ms timeout against a {:?} endpoint must be recorded as a failure",
            SERVER_LATENCY
        );
        let health = tight.jwks_health();
        assert!(
            !health.ever_loaded,
            "no keyset can load when the timeout is below the endpoint's latency: {health:?}"
        );
        assert!(tight.readiness().is_err(), "{health:?}");
        tight.stop_background_refresh();
    }

    // (b) Configured above it: the same endpoint now works. Under the old constant this outcome
    //     was unreachable no matter how the deployment was configured.
    {
        let _env = EnvOverride::set("5000");
        let generous = JwksBearerProvider::new(&url);
        assert!(
            wait_until(Duration::from_secs(20), || generous
                .jwks_health()
                .ever_loaded),
            "a 5000ms timeout must let a {:?} endpoint through",
            SERVER_LATENCY
        );
        let health = generous.jwks_health();
        assert_eq!(health.keys_cached, 1, "{health:?}");
        assert!(generous.readiness().is_ok(), "{health:?}");
        assert!(validate(&generous, &token(secret, kid)));
        generous.stop_background_refresh();
    }
}

/// The per-provider builder overrides the process-wide configured default.
#[test]
fn provider_builder_overrides_the_configured_timeout() {
    let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());

    let secret = b"builder-override-secret";
    let kid = "builder-override-kid";
    let url = start_slow_jwks_server(jwks(secret, kid));

    // Environment says "too tight for this endpoint"; the builder says otherwise.
    let _env = EnvOverride::set("100");
    let provider = JwksBearerProvider::new(&url).fetch_timeout(Duration::from_millis(5000));

    // Join the background thread so the refresh under test runs on this thread with the builder's
    // timeout rather than the environment's.
    provider.stop_background_refresh();

    assert!(
        validate(&provider, &token(secret, kid)),
        "the builder's timeout must be the one the refresh path uses"
    );
    let health = provider.jwks_health();
    assert!(health.ever_loaded, "{health:?}");
    assert_eq!(health.keys_cached, 1, "{health:?}");
}

/// Guard the default itself: nothing should quietly move it back under a TLS handshake.
#[test]
fn default_timeout_is_tls_sized() {
    assert!(
        DEFAULT_JWKS_FETCH_TIMEOUT > Duration::from_millis(200),
        "the default must not regress to the plaintext-era 200ms budget"
    );
}
