//! JWKS fetch failures must be loud and diagnosable.
//!
//! Regression tests for the failure mode where `fetch_get_text_with_retry` logged every JWKS
//! fetch error at `debug!` and returned `None`. At default log levels that made a JWKS endpoint
//! that had never once answered indistinguishable from a healthy one: the refresh silently never
//! succeeded, every token validation failed with a bare 401, and nothing in the logs named DNS,
//! TLS, the edge or the upstream as the cause.
//!
//! These tests assert three things:
//!
//! 1. a failed fetch is reported at `WARN`/`ERROR`, not `DEBUG`, with the URL, attempt count,
//!    elapsed time and underlying error;
//! 2. *never succeeded* (fatal — no keys at all) is reported differently from *refresh failed but
//!    a cached keyset is still being served* (degraded); and
//! 3. the health/readiness surface reflects a provider that has never fetched a keyset.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use brrtrouter::dispatcher::HeaderVec;
use brrtrouter::http::{fetch_get_text_with_retry, HttpFetchOptions};
use brrtrouter::router::ParamVec;
use brrtrouter::security::{JwksBearerProvider, SecurityProvider, SecurityRequest};
use brrtrouter::spec::SecurityScheme;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use tracing::Level;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Registry;

// ---------------------------------------------------------------------------
// In-process log capture
//
// The provider refreshes JWKS on background threads, so a thread-local
// `tracing::subscriber::set_default` would miss exactly the events under test. Each integration
// test file is its own process, so a single global subscriber here is safe; tests disambiguate
// their events by using a unique JWKS URL.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct CapturedEvent {
    level: Level,
    text: String,
}

#[derive(Default)]
struct FieldText(String);

impl tracing::field::Visit for FieldText {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let _ = write!(self.0, " {}={:?}", field.name(), value);
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let _ = write!(self.0, " {}={}", field.name(), value);
    }
}

struct CaptureLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for CaptureLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut text = FieldText::default();
        event.record(&mut text);
        if let Ok(mut events) = self.events.lock() {
            events.push(CapturedEvent {
                level: *event.metadata().level(),
                text: text.0,
            });
        }
    }
}

static EVENTS: OnceLock<Arc<Mutex<Vec<CapturedEvent>>>> = OnceLock::new();

fn events() -> &'static Arc<Mutex<Vec<CapturedEvent>>> {
    EVENTS.get_or_init(|| {
        let events = Arc::new(Mutex::new(Vec::new()));
        let layer = CaptureLayer {
            events: Arc::clone(&events),
        };
        tracing::subscriber::set_global_default(Registry::default().with(layer))
            .expect("this test binary installs exactly one global subscriber");
        events
    })
}

fn captured() -> Vec<CapturedEvent> {
    events().lock().unwrap().clone()
}

/// Poll captured events until one matches, or the deadline passes.
fn wait_for(
    timeout: Duration,
    predicate: impl Fn(&CapturedEvent) -> bool,
) -> Option<CapturedEvent> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(found) = captured().into_iter().find(&predicate) {
            return Some(found);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

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
    let claims = serde_json::json!({ "sub": "diagnostics", "exp": now + 300 });
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

/// Serve `body` once, then fail every subsequent request with a 500.
///
/// Models the operationally interesting case: the router loaded a keyset at startup and the
/// issuer then became unreachable.
fn start_flaky_jwks_server(body: String) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let served = Arc::clone(&requests);

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0_u8; 2048];
            let _ = stream.read(&mut buf);
            let index = served.fetch_add(1, Ordering::SeqCst);
            let response = if index == 0 {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            } else {
                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            };
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    (
        format!("http://{address}/flaky-issuer/.well-known/jwks.json"),
        requests,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The transport helper must not swallow failures into `debug!` any more.
#[test]
fn fetch_failure_surfaces_at_warn_and_error_not_debug() {
    events();

    // Port 1 is reserved and never listening — a stand-in for the bad DNS answer / blocked
    // handshake / unreachable edge that this logging exists to tell apart.
    let url = "http://127.0.0.1:1/never-answers/jwks.json";
    let options = HttpFetchOptions {
        timeout: Duration::from_millis(150),
        max_body_bytes: 4096,
        extra_headers: Vec::new(),
    };

    assert!(fetch_get_text_with_retry(url, &options, 2).is_none());

    let relevant: Vec<CapturedEvent> = captured()
        .into_iter()
        .filter(|event| event.text.contains("never-answers/jwks.json"))
        .collect();

    assert!(
        !relevant.is_empty(),
        "a failed fetch must be logged at all; it used to vanish into debug!"
    );
    assert!(
        relevant.iter().any(|event| event.level == Level::WARN),
        "each failed attempt must be reported at WARN, got {relevant:?}"
    );

    let fatal = relevant
        .iter()
        .find(|event| event.level == Level::ERROR)
        .expect("exhausting every attempt must be reported at ERROR");

    for field in ["attempts=", "elapsed_ms=", "timeout_ms=", "error="] {
        assert!(
            fatal.text.contains(field),
            "operators need {field} in the failure log, got {fatal:?}"
        );
    }
}

/// A provider that has never loaded a keyset is fatal, not degraded, and must say so — and must
/// not report itself ready.
#[test]
fn provider_that_never_loaded_a_keyset_logs_error_and_is_not_ready() {
    events();

    let url = "http://127.0.0.1:1/dead-issuer/.well-known/jwks.json";
    let provider = JwksBearerProvider::new(url).fetch_timeout(Duration::from_millis(120));

    // Drive the request path; the refresh it triggers can only fail.
    let _ = validate(&provider, &token(b"never-loaded-secret", "kid-1"));

    let fatal = wait_for(Duration::from_secs(10), |event| {
        event.level == Level::ERROR
            && event.text.contains("dead-issuer")
            && event.text.contains("ALL JWT validation will fail")
    })
    .expect("a JWKS refresh that has never succeeded must be ERROR, naming the impact");

    // The message has to be actionable, not just noisy.
    assert!(
        fatal.text.contains("BRRTR_JWKS_FETCH_TIMEOUT_MS"),
        "failure log should point at the timeout knob: {fatal:?}"
    );
    assert!(
        fatal.text.contains("timeout_ms=") && fatal.text.contains("attempts="),
        "failure log should report the timeout and attempt count actually used: {fatal:?}"
    );

    let health = provider.jwks_health();
    assert!(!health.ever_loaded, "no keyset has ever loaded: {health:?}");
    assert!(health.fetch_failure > 0, "{health:?}");
    assert!(!health.is_ready(), "{health:?}");
    assert!(
        !health.is_degraded(),
        "never-loaded is fatal, not degraded: {health:?}"
    );

    let readiness = provider
        .readiness()
        .expect_err("a provider with no key material must not report ready");
    assert!(
        readiness.contains("never been fetched"),
        "readiness reason must be diagnosable: {readiness}"
    );

    provider.stop_background_refresh();
}

/// A refresh that fails while a cached keyset is still being served is *degraded*: existing
/// tokens keep working, so it warns rather than errors and stays ready.
#[test]
fn refresh_failure_over_a_cached_keyset_is_degraded_not_fatal() {
    events();

    let secret = b"degraded-secret";
    let kid = "degraded-kid";
    let (url, _requests) = start_flaky_jwks_server(jwks(secret, kid));

    let provider = JwksBearerProvider::new(&url)
        .cache_ttl(Duration::from_millis(50))
        .fetch_timeout(Duration::from_millis(500));

    // Wait for the startup refresh (which gets the single 200) before stopping the background
    // thread, so the rest of the test is deterministic.
    let deadline = Instant::now() + Duration::from_secs(10);
    while !provider.jwks_health().ever_loaded && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    provider.stop_background_refresh();

    let loaded = provider.jwks_health();
    assert!(
        loaded.ever_loaded,
        "startup refresh should succeed: {loaded:?}"
    );
    assert_eq!(loaded.keys_cached, 1, "{loaded:?}");
    assert!(provider.readiness().is_ok(), "{loaded:?}");

    // Let the cache expire so the next validation triggers a refresh — which now gets a 500.
    thread::sleep(Duration::from_millis(150));
    assert!(
        validate(&provider, &token(secret, kid)),
        "a failed refresh must not invalidate the keys we already hold"
    );

    let degraded = wait_for(Duration::from_secs(10), |event| {
        event.level == Level::WARN && event.text.contains("DEGRADED")
    })
    .expect("a failed refresh over a live cache must WARN as degraded, not ERROR as fatal");
    assert!(
        degraded.text.contains("cached_keys="),
        "degraded log should say how much key material is still usable: {degraded:?}"
    );

    let health = provider.jwks_health();
    assert!(health.ever_loaded, "{health:?}");
    assert!(health.consecutive_failures > 0, "{health:?}");
    assert!(health.is_degraded(), "{health:?}");
    assert!(
        provider.readiness().is_ok(),
        "degraded must stay in the load balancer: pulling the pod would turn a partial outage \
         into a total one"
    );
}
