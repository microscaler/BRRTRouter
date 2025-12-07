use brrtrouter::security::{JwksBearerProvider, SecurityProvider, SecurityRequest};
use brrtrouter::{dispatcher::HeaderVec, router::ParamVec, spec::SecurityScheme};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use http::Method;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// Create a mock JWKS server response for testing
fn create_mock_jwks(secret: &[u8]) -> String {
    use base64::Engine;
    let k = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);
    serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string()
}

/// Generate a JWT token for testing
fn make_token(secret: &[u8], kid: &str, exp_secs: i64) -> String {
    let header = Header {
        kid: Some(kid.to_string()),
        alg: Algorithm::HS256,
        ..Default::default()
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = json!({
        "iss": "https://issuer.example",
        "aud": "my-audience",
        "exp": now + exp_secs,
        "scope": "read write"
    });
    jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
}

/// Mock HTTP server for JWKS endpoint
struct MockJwksServer {
    port: u16,
    jwks: String,
}

impl MockJwksServer {
    fn new(jwks: String) -> Self {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let jwks_clone = jwks.clone();

        thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    let mut buffer = [0; 1024];
                    let _ = stream.read(&mut buffer);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        jwks_clone.len(),
                        jwks_clone
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                }
            }
        });

        // Give server a moment to start
        std::thread::sleep(Duration::from_millis(100));

        Self { port, jwks }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

fn create_provider(jwks_url: &str) -> JwksBearerProvider {
    JwksBearerProvider::new(jwks_url)
        .issuer("https://issuer.example".to_string())
        .audience("my-audience".to_string())
        .claims_cache_size(1000)
}

fn create_request<'a>(
    token: &str,
    headers: &'a mut HeaderVec,
    query: &'a ParamVec,
    cookies: &'a HeaderVec,
) -> SecurityRequest<'a> {
    headers.clear();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    SecurityRequest {
        headers,
        query,
        cookies,
    }
}

/// Benchmark cache hit performance (token already in cache)
fn bench_cache_hit(c: &mut Criterion) {
    let secret = b"supersecret";
    let jwks = create_mock_jwks(secret);
    let server = MockJwksServer::new(jwks);
    let provider = create_provider(&server.url());
    let token = make_token(secret, "k1", 3600);
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Warm up cache
    let mut headers = HeaderVec::new();
    let query = ParamVec::new();
    let cookies = HeaderVec::new();
    let req = create_request(&token, &mut headers, &query, &cookies);
    let _ = provider.validate(&scheme, &[], &req);

    c.bench_function("jwt_cache_hit", |b| {
        let mut headers = HeaderVec::new();
        let query = ParamVec::new();
        let cookies = HeaderVec::new();
        b.iter(|| {
            let req = create_request(&token, &mut headers, &query, &cookies);
            black_box(provider.validate(black_box(&scheme), black_box(&[]), black_box(&req)))
        })
    });
}

/// Benchmark cache miss performance (token not in cache, requires decode)
fn bench_cache_miss(c: &mut Criterion) {
    let secret = b"supersecret";
    let jwks = create_mock_jwks(secret);
    let server = MockJwksServer::new(jwks);
    let provider = create_provider(&server.url());
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    c.bench_function("jwt_cache_miss", |b| {
        let mut headers = HeaderVec::new();
        let query = ParamVec::new();
        let cookies = HeaderVec::new();
        // Generate new token each iteration to force cache miss
        b.iter(|| {
            let token = make_token(secret, "k1", 3600);
            let req = create_request(&token, &mut headers, &query, &cookies);
            black_box(provider.validate(black_box(&scheme), black_box(&[]), black_box(&req)))
        })
    });
}

/// Benchmark concurrent cache access (lock contention)
fn bench_concurrent_access(c: &mut Criterion) {
    let secret = b"supersecret";
    let jwks = create_mock_jwks(secret);
    let server = MockJwksServer::new(jwks);
    let provider = Arc::new(create_provider(&server.url()));
    let token = make_token(secret, "k1", 3600);
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Warm up cache
    let mut headers = HeaderVec::new();
    let query = ParamVec::new();
    let cookies = HeaderVec::new();
    let req = create_request(&token, &mut headers, &query, &cookies);
    let _ = provider.validate(&scheme, &[], &req);

    let mut group = c.benchmark_group("jwt_concurrent");
    for threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(threads),
            threads,
            |b, &num_threads| {
                b.iter(|| {
                    use std::thread;
                    let mut handles = Vec::new();
                    for _ in 0..num_threads {
                        let provider = provider.clone();
                        let token = token.clone();
                        let scheme = scheme.clone();
                        let handle = thread::spawn(move || {
                            let mut headers = HeaderVec::new();
                            let query = ParamVec::new();
                            let cookies = HeaderVec::new();
                            for _ in 0..100 {
                                let req = create_request(&token, &mut headers, &query, &cookies);
                                black_box(provider.validate(&scheme, &[], &req));
                            }
                        });
                        handles.push(handle);
                    }
                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );
    }
    group.finish();
}

/// Benchmark cache eviction impact (cache at capacity)
fn bench_cache_eviction(c: &mut Criterion) {
    let secret = b"supersecret";
    let jwks = create_mock_jwks(secret);
    let server = MockJwksServer::new(jwks);
    
    let mut group = c.benchmark_group("jwt_cache_eviction");
    for cache_size in [10, 100, 1000].iter() {
        let provider = JwksBearerProvider::new(&server.url())
            .issuer("https://issuer.example".to_string())
            .audience("my-audience".to_string())
            .claims_cache_size(*cache_size);
        
        let scheme = SecurityScheme::Http {
            scheme: "bearer".to_string(),
            bearer_format: None,
            description: None,
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(cache_size),
            cache_size,
            |b, &_cache_size| {
                // Fill cache to capacity
                let mut headers = HeaderVec::new();
                let query = ParamVec::new();
                let cookies = HeaderVec::new();
                for _ in 0..*cache_size {
                    let token = make_token(secret, "k1", 3600);
                    let req = create_request(&token, &mut headers, &query, &cookies);
                    let _ = provider.validate(&scheme, &[], &req);
                }

                // Now benchmark with cache at capacity (evictions will occur)
                b.iter(|| {
                    let token = make_token(secret, "k1", 3600);
                    let req = create_request(&token, &mut headers, &query, &cookies);
                    black_box(provider.validate(black_box(&scheme), black_box(&[]), black_box(&req)))
                })
            },
        );
    }
    group.finish();
}

/// Benchmark cache statistics retrieval
fn bench_cache_stats(c: &mut Criterion) {
    let secret = b"supersecret";
    let jwks = create_mock_jwks(secret);
    let server = MockJwksServer::new(jwks);
    let provider = create_provider(&server.url());
    let token = make_token(secret, "k1", 3600);
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Generate some cache activity
    let mut headers = HeaderVec::new();
    let query = ParamVec::new();
    let cookies = HeaderVec::new();
    for _ in 0..100 {
        let req = create_request(&token, &mut headers, &query, &cookies);
        let _ = provider.validate(&scheme, &[], &req);
    }

    c.bench_function("jwt_cache_stats", |b| {
        b.iter(|| {
            black_box(provider.cache_stats())
        })
    });
}

criterion_group!(
    benches,
    bench_cache_hit,
    bench_cache_miss,
    bench_concurrent_access,
    bench_cache_eviction,
    bench_cache_stats
);
criterion_main!(benches);

