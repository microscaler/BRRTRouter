use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::middleware::MetricsMiddleware;
use brrtrouter::router::Router;
use brrtrouter::runtime_config::RuntimeConfig;
use brrtrouter::security::{JwksBearerProvider, RemoteApiKeyProvider};
use brrtrouter::server::AppService;
use brrtrouter::server::HttpServer;
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{BearerJwtProvider, OAuth2Provider, SecurityProvider, SecurityRequest};
use clap::Parser;
use pet_store::registry;
use std::fs;
use std::io;
use std::path::PathBuf;
use tikv_jemallocator::Jemalloc;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct AppConfig {
    security: Option<SecurityConfig>,
    http: Option<HttpConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct SecurityConfig {
    api_keys: Option<std::collections::HashMap<String, ApiKeyConfig>>, // by scheme name
    remote_api_keys: Option<std::collections::HashMap<String, RemoteApiKeyConfig>>, // by scheme name
    bearer: Option<BearerConfig>,
    oauth2: Option<OAuth2Config>,
    jwks: Option<std::collections::HashMap<String, JwksConfig>>, // by scheme name
    propelauth: Option<PropelAuthConfig>,                        // global PropelAuth config
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct ApiKeyConfig {
    key: Option<String>,         // static key for simple validations
    header_name: Option<String>, // override header for header-based keys
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct RemoteApiKeyConfig {
    verify_url: String,
    timeout_ms: Option<u64>,
    header_name: Option<String>,
    cache_ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct BearerConfig {
    signature: Option<String>,
    cookie_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct OAuth2Config {
    signature: Option<String>,
    cookie_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct JwksConfig {
    jwks_url: String,
    iss: Option<String>,
    aud: Option<String>,
    leeway_secs: Option<u64>,
    cache_ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct PropelAuthConfig {
    // Base auth URL from PropelAuth project settings, e.g. https://auth.yourdomain.com
    auth_url: String,
    // Optional overrides
    audience: Option<String>,
    issuer: Option<String>,
    jwks_url: Option<String>,
    leeway_secs: Option<u64>,
    cache_ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct HttpConfig {
    keep_alive: Option<bool>,
    timeout_secs: Option<u64>,
    max_requests: Option<u64>,
}

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value = "./doc/openapi.yaml")]
    spec: PathBuf,
    #[arg(long)]
    static_dir: Option<PathBuf>,
    #[arg(long, default_value = "./doc")]
    doc_dir: PathBuf,
    // Accept compatibility flags used by repo scripts; currently informational
    #[arg(long, default_value_t = false)]
    hot_reload: bool,
    #[arg(long)]
    test_api_key: Option<String>,
    #[arg(long, default_value = "./config/config.yaml")]
    config: PathBuf,
}

fn main() -> io::Result<()> {
    // Initialize structured logging early so all subsequent logs (including request logs)
    // are emitted and scraped by Promtail/Loki. Honors RUST_LOG via EnvFilter.
    if let Err(e) =
        brrtrouter::otel::init_logging_with_config(&brrtrouter::otel::LogConfig::from_env())
    {
        eprintln!("[logging][error] failed to init tracing subscriber: {e}");
    }

    let args = Args::parse();
    // configure coroutine stack size
    let config = RuntimeConfig::from_env();
    may::config().set_stack_size(config.stack_size);
    // Load OpenAPI spec and create router
    // Resolve relative specs against the crate directory so launches from other CWDs work
    let spec_path = if args.spec.is_relative() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        base.join(args.spec)
    } else {
        args.spec.clone()
    };
    if args.hot_reload {
        println!("[info] hot-reload requested (handled internally by service watcher if enabled)");
    }
    if let Some(k) = &args.test_api_key {
        println!("[info] test-api-key provided ({} chars)", k.len());
    }
    let (routes, schemes, _slug) = brrtrouter::spec::load_spec_full(spec_path.to_str().unwrap())
        .expect("failed to load OpenAPI spec");
    let _router = Router::new(routes.clone());
    // Create router and dispatcher
    let mut dispatcher = Dispatcher::new();

    // Create dispatcher and middleware
    let metrics = std::sync::Arc::new(MetricsMiddleware::new());
    dispatcher.add_middleware(metrics.clone());
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }

    // Start the HTTP server on port 8080, binding to 127.0.0.1 if BRRTR_LOCAL is
    // set for local testing.
    // This returns a coroutine JoinHandle; we join on it to keep the server running
    let router = std::sync::Arc::new(std::sync::RwLock::new(Router::new(routes)));
    // Dump initial route table
    router.read().unwrap().dump_routes();
    let dispatcher = std::sync::Arc::new(std::sync::RwLock::new(dispatcher));
    let mut service = AppService::new(
        router,
        dispatcher,
        schemes,
        spec_path.clone(),
        args.static_dir.clone(),
        Some(args.doc_dir.clone()),
    );
    service.set_metrics_middleware(metrics);

    // Load application config (YAML). If the file exists but is invalid, fail fast with a clear error.
    // Only a missing file results in defaulting.
    let app_config: AppConfig = match fs::read_to_string(&args.config) {
        Ok(s) => match serde_yaml::from_str::<AppConfig>(&s) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!(
                    "[config][error] Failed to parse {}: {}",
                    args.config.display(),
                    e
                );
                return Err(io::Error::other(format!(
                    "Invalid configuration file {}: {}",
                    args.config.display(),
                    e
                )));
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            println!(
                "[config] {} not found; continuing with defaults",
                args.config.display()
            );
            AppConfig::default()
        }
        Err(e) => {
            return Err(io::Error::other(format!(
                "Failed to read configuration file {}: {}",
                args.config.display(),
                e
            )));
        }
    };
    // Log startup context and config (sanitized)
    println!("[startup] spec_path={}", spec_path.display());
    if let Some(sd) = &args.static_dir {
        println!("[startup] static_dir={}", sd.display());
    }
    println!("[startup] doc_dir={}", args.doc_dir.display());
    match serde_yaml::to_string(&app_config) {
        Ok(y) => println!("[config]\n{}", y),
        Err(_) => println!("[config] <failed to serialize config>"),
    }

    // Keep-Alive from config (default ON for testing in generated app)
    let (enable, timeout, max) = match app_config.http.as_ref() {
        Some(http) => (
            http.keep_alive.unwrap_or(true),
            http.timeout_secs.unwrap_or(5),
            http.max_requests.unwrap_or(1000),
        ),
        None => (true, 5, 1000),
    };
    service.set_keep_alive(enable, timeout, max);

    // Register security providers from config first; if not found, fall back to env/CLI defaults
    {
        // Simple static ApiKey provider for header/query/cookie
        struct StaticApiKeyProvider {
            key: String,
            header_override: Option<String>,
        }
        impl SecurityProvider for StaticApiKeyProvider {
            fn validate(
                &self,
                scheme: &SecurityScheme,
                _scopes: &[String],
                req: &SecurityRequest,
            ) -> bool {
                match scheme {
                    SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                        "header" => {
                            let target = self
                                .header_override
                                .as_ref()
                                .map(|s| s.as_str())
                                .unwrap_or_else(|| name);
                            req.headers
                                .get(&target.to_ascii_lowercase())
                                .map(|s| s.as_str())
                                == Some(self.key.as_str())
                        }
                        "query" => req.query.get(name) == Some(&self.key),
                        "cookie" => req.cookies.get(name) == Some(&self.key),
                        _ => false,
                    },
                    _ => false,
                }
            }
        }

        let sec_cfg = app_config.security.as_ref();
        for (scheme_name, scheme) in service.security_schemes.clone() {
            match scheme {
                SecurityScheme::ApiKey { .. } => {
                    let mut registered = false;
                    if let Some(cfgs) = sec_cfg.and_then(|s| s.remote_api_keys.as_ref()) {
                        if let Some(cfg) = cfgs.get(&scheme_name) {
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
                            println!("[auth] register RemoteApiKeyProvider scheme={} url={} header={} timeout_ms={:?} ttl_s={:?}", scheme_name, cfg.verify_url, cfg.header_name.clone().unwrap_or_else(|| "(default X-API-Key)".into()), cfg.timeout_ms, cfg.cache_ttl_secs);
                            service.register_security_provider(
                                &scheme_name,
                                std::sync::Arc::new(provider),
                            );
                            registered = true;
                        }
                    }
                    if !registered {
                        if let Some(cfgs) = sec_cfg.and_then(|s| s.api_keys.as_ref()) {
                            if let Some(cfg) = cfgs.get(&scheme_name) {
                                if let Some(key) = cfg.key.clone() {
                                    println!("[auth] register StaticApiKeyProvider scheme={} header_override={:?} key_len={}", scheme_name, cfg.header_name, key.len());
                                    service.register_security_provider(
                                        &scheme_name,
                                        std::sync::Arc::new(StaticApiKeyProvider {
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
                        // Fallback to env/CLI
                        let fallback = std::env::var("BRRTR_API_KEY")
                            .ok()
                            .or_else(|| args.test_api_key.clone())
                            .unwrap_or_else(|| "test123".to_string());
                        println!("[auth] register StaticApiKeyProvider scheme={} from=fallback key_len={}", scheme_name, fallback.len());
                        service.register_security_provider(
                            &scheme_name,
                            std::sync::Arc::new(StaticApiKeyProvider {
                                key: fallback,
                                header_override: None,
                            }),
                        );
                    }
                }
                SecurityScheme::Http { ref scheme, .. }
                    if scheme.eq_ignore_ascii_case("bearer") =>
                {
                    // Prefer PropelAuth (if configured) → JWKS per-scheme → signature-based mock
                    if let Some(pa) = sec_cfg.and_then(|s| s.propelauth.as_ref()) {
                        let jwks_url = pa.jwks_url.clone().unwrap_or_else(|| {
                            let base = pa.auth_url.trim_end_matches('/');
                            format!("{}/.well-known/jwks.json", base)
                        });
                        let mut p = JwksBearerProvider::new(&jwks_url);
                        let issuer_opt: Option<&str> =
                            pa.issuer.as_deref().or_else(|| Some(pa.auth_url.as_str()));
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
                        println!("[auth] register JwksBearerProvider scheme={} source=propelauth jwks_url={} iss={:?} aud={:?}", scheme_name, jwks_url, pa.issuer, pa.audience);
                        service.register_security_provider(&scheme_name, std::sync::Arc::new(p));
                        continue;
                    }
                    // Next, check per-scheme JWKS mapping
                    if let Some(jwks_map) = sec_cfg.and_then(|s| s.jwks.as_ref()) {
                        if let Some(jwks) = jwks_map.get(&scheme_name) {
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
                            println!("[auth] register JwksBearerProvider scheme={} source=per-scheme jwks_url={} iss={:?} aud={:?}", scheme_name, jwks.jwks_url, jwks.iss, jwks.aud);
                            service
                                .register_security_provider(&scheme_name, std::sync::Arc::new(p));
                            continue;
                        }
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
                    println!("[auth] register BearerJwtProvider scheme={} source=mock signature_len={} cookie={:?}", scheme_name, sig_len, cookie_opt);
                    service.register_security_provider(&scheme_name, std::sync::Arc::new(p));
                }
                SecurityScheme::OAuth2 { .. } => {
                    // Prefer PropelAuth (if configured) → JWKS under same scheme → signature-based mock
                    if let Some(pa) = sec_cfg.and_then(|s| s.propelauth.as_ref()) {
                        let jwks_url = pa.jwks_url.clone().unwrap_or_else(|| {
                            let base = pa.auth_url.trim_end_matches('/');
                            format!("{}/.well-known/jwks.json", base)
                        });
                        let mut p = JwksBearerProvider::new(&jwks_url);
                        let issuer_opt: Option<&str> =
                            pa.issuer.as_deref().or_else(|| Some(pa.auth_url.as_str()));
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
                        println!("[auth] register JwksBearerProvider scheme={} source=propelauth jwks_url={} iss={:?} aud={:?}", scheme_name, jwks_url, pa.issuer, pa.audience);
                        service.register_security_provider(&scheme_name, std::sync::Arc::new(p));
                        continue;
                    }
                    // Next, check per-scheme JWKS mapping
                    if let Some(jwks_map) = sec_cfg.and_then(|s| s.jwks.as_ref()) {
                        if let Some(jwks) = jwks_map.get(&scheme_name) {
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
                            println!("[auth] register JwksBearerProvider scheme={} source=per-scheme jwks_url={} iss={:?} aud={:?}", scheme_name, jwks.jwks_url, jwks.iss, jwks.aud);
                            service
                                .register_security_provider(&scheme_name, std::sync::Arc::new(p));
                            continue;
                        }
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
                    println!("[auth] register OAuth2Provider scheme={} source=mock signature_len={} cookie={:?}", scheme_name, sig_len, cookie_opt);
                    service.register_security_provider(&scheme_name, std::sync::Arc::new(p));
                }
                _ => {}
            }
        }
    }
    let addr = if std::env::var("BRRTR_LOCAL").is_ok() {
        "127.0.0.1:8080"
    } else {
        "0.0.0.0:8080"
    };
    println!("🚀 pet_store example server listening on {addr}");
    let server = HttpServer(service).start(addr).map_err(io::Error::other)?;
    println!("Server started successfully on {addr}");

    server
        .join()
        .map_err(|e| io::Error::other(format!("Server encountered an error: {e:?}")))?;
    Ok(())
}
