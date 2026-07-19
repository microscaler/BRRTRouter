//! Fix B: collapse generated/impl `main.rs` boilerplate into the library.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::dispatcher::Dispatcher;
use crate::middleware::MetricsMiddleware;
use crate::router::Router;
use crate::runtime_config::RuntimeConfig;
use crate::spec::RouteMeta;

use super::app_config::{load_app_config, AppConfig};
use super::cors_setup::build_cors_middleware;
use super::security_setup::register_security_from_config;
use super::{AppService, HttpServer};

/// Paths and flags passed from a slim service `main`.
#[derive(Debug, Clone)]
pub struct RunAppArgs {
    pub spec: PathBuf,
    pub config: PathBuf,
    pub doc_dir: PathBuf,
    pub static_dir: Option<PathBuf>,
    pub hot_reload: bool,
    pub test_api_key: Option<String>,
    /// Crate root for resolving relative `--spec` paths.
    pub manifest_dir: PathBuf,
    /// When `config.yaml` and `PORT` are unset.
    pub default_port: u16,
    /// Banner label in startup log.
    pub service_name: String,
}

/// Service-specific startup hooks (auth client init, extra runtime metrics, DB warm, etc.).
#[derive(Default)]
pub struct RunAppHooks {
    /// Called after config is loaded, before route registration.
    pub on_config_loaded: Option<Box<dyn FnOnce(&AppConfig)>>,
    /// Extra Prometheus scrape text (e.g. a companion runtime's metrics).
    pub extra_prometheus: Option<Arc<dyn Fn() -> String + Send + Sync>>,
    /// Called on the main OS thread immediately before binding the listen socket.
    pub before_listen: Option<Box<dyn FnOnce()>>,
}

/// Registers gen + impl handlers. Must be `unsafe` because it spawns coroutines.
pub type RegisterHandlersFn = unsafe fn(dispatcher: &mut Dispatcher, routes: &[RouteMeta]);

/// Builder for service startup (Fix B).
#[derive(Default)]
pub struct RunAppBuilder {
    args: Option<RunAppArgs>,
    hooks: RunAppHooks,
    register: Option<RegisterHandlersFn>,
}

impl RunAppBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn args(mut self, args: RunAppArgs) -> Self {
        self.args = Some(args);
        self
    }

    pub fn hooks(mut self, hooks: RunAppHooks) -> Self {
        self.hooks = hooks;
        self
    }

    pub fn register(mut self, register: RegisterHandlersFn) -> Self {
        self.register = Some(register);
        self
    }

    /// Run the full service bootstrap: config, CORS, auth, HTTP server.
    pub fn run(self) -> io::Result<()> {
        let args = self
            .args
            .ok_or_else(|| io::Error::other("RunAppBuilder: args required"))?;
        let register = self
            .register
            .ok_or_else(|| io::Error::other("RunAppBuilder: register handler required"))?;
        let hooks = self.hooks;

        if let Err(e) = crate::otel::init_logging_with_config(&crate::otel::LogConfig::from_env()) {
            eprintln!("[logging][error] failed to init tracing subscriber: {e}");
        }

        let runtime = RuntimeConfig::from_env();
        may::config().set_stack_size(runtime.stack_size);
        may::config().set_workers(runtime.may_workers);

        let spec_path = resolve_path(&args.manifest_dir, &args.spec);
        if args.hot_reload {
            println!(
                "[info] hot-reload requested (handled internally by service watcher if enabled)"
            );
        }
        if let Some(k) = &args.test_api_key {
            println!("[info] test-api-key provided ({} chars)", k.len());
        }

        let app_config = load_app_config(&args.config)?;
        if let Some(cb) = hooks.on_config_loaded {
            cb(&app_config);
        }

        let spec_str = spec_path
            .to_str()
            .ok_or_else(|| io::Error::other("OpenAPI spec path contains invalid UTF-8"))?;
        let (routes, schemes, _slug) = crate::spec::load_spec_full(spec_str)
            .map_err(|e| io::Error::other(format!("failed to load OpenAPI spec: {e}")))?;

        let mut dispatcher = Dispatcher::new();
        let metrics = Arc::new(MetricsMiddleware::new());
        dispatcher.add_middleware(metrics.clone());

        let memory = Arc::new(crate::middleware::MemoryMiddleware::new());
        crate::middleware::memory::start_memory_monitor(memory.clone());

        if let Some(cors) = build_cors_middleware(&app_config, &routes, metrics.clone()) {
            dispatcher.add_middleware(cors);
        }

        unsafe {
            register(&mut dispatcher, &routes);
        }

        let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes.clone())));
        router.load().dump_routes();
        let dispatcher = Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher));
        let mut service = AppService::new(
            router,
            dispatcher,
            schemes,
            spec_path.clone(),
            args.static_dir.clone(),
            Some(args.doc_dir.clone()),
        );

        let compiled_count = service.precompile_schemas(&routes);
        println!("[startup] precompiled {compiled_count} JSON schema validators");

        service.set_metrics_middleware(metrics);
        if let Some(extra) = hooks.extra_prometheus {
            service.set_extra_prometheus(Some(extra));
        }
        service.set_memory_middleware(memory);

        log_startup_context(
            &args,
            &spec_path,
            &app_config,
            runtime.stack_size,
            runtime.may_workers,
            routes.len(),
        );

        let (enable, timeout, max) = match app_config.http.as_ref() {
            Some(http) => (
                http.keep_alive.unwrap_or(true),
                http.timeout_secs.unwrap_or(5),
                http.max_requests.unwrap_or(1000),
            ),
            None => (true, 5, 1000),
        };
        service.set_keep_alive(enable, timeout, max);

        register_security_from_config(&mut service, &app_config, args.test_api_key.as_deref());

        let port = app_config
            .port
            .or_else(|| {
                std::env::var("PORT")
                    .ok()
                    .and_then(|p| p.parse::<u16>().ok())
            })
            .unwrap_or(args.default_port);
        let addr = if std::env::var("BRRTR_LOCAL").is_ok() {
            format!("127.0.0.1:{port}")
        } else {
            format!("0.0.0.0:{port}")
        };

        println!(
            "🚀 {} example server listening on {addr}",
            args.service_name
        );

        if let Some(warm) = hooks.before_listen {
            warm();
        }

        let server = HttpServer(service).start(&addr).map_err(io::Error::other)?;
        println!("Server started successfully on {addr}");

        server
            .run_until_shutdown()
            .map_err(|e| io::Error::other(format!("Server encountered an error: {e:?}")))
    }
}

fn resolve_path(manifest_dir: &Path, path: &Path) -> PathBuf {
    if path.is_relative() {
        manifest_dir.join(path)
    } else {
        path.to_path_buf()
    }
}

fn log_startup_context(
    args: &RunAppArgs,
    spec_path: &Path,
    app_config: &AppConfig,
    stack_size: usize,
    may_workers: usize,
    routes_count: usize,
) {
    println!("[startup] spec_path={}", spec_path.display());
    if let Some(sd) = &args.static_dir {
        println!("[startup] static_dir={}", sd.display());
    }
    println!("[startup] doc_dir={}", args.doc_dir.display());
    println!(
        "[startup] stack_size={stack_size} may_workers={may_workers} routes_count={routes_count} hot_reload={}",
        args.hot_reload
    );
    match serde_yaml::to_string(app_config) {
        Ok(y) => println!("[config]\n{y}"),
        Err(_) => println!("[config] <failed to serialize config>"),
    }
}
