use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::middleware::MetricsMiddleware;
use brrtrouter::router::Router;
use brrtrouter::runtime_config::RuntimeConfig;
use brrtrouter::server::AppService;
use brrtrouter::server::HttpServer;
use clap::Parser;
use pet_store::registry;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value = "./doc/openapi.yaml")]
    spec: PathBuf,
    #[arg(long)]
    static_dir: Option<PathBuf>,
    #[arg(long, default_value = "./doc")]
    doc_dir: PathBuf,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    // configure coroutine stack size
    let config = RuntimeConfig::from_env();
    may::config().set_stack_size(config.stack_size);
    // Load OpenAPI spec and create router
    let (routes, _slug) = brrtrouter::spec::load_spec(args.spec.to_str().unwrap())
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
    let dispatcher = std::sync::Arc::new(std::sync::RwLock::new(dispatcher));
    let mut service = AppService::new(
        router,
        dispatcher,
        HashMap::new(),
        args.spec.clone(),
        args.static_dir.clone(),
        Some(args.doc_dir.clone()),
    );
    service.set_metrics_middleware(metrics);
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
