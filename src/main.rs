use brrtrouter::dispatcher::Dispatcher;
// use brrrouter::registry;
use brrtrouter::server::AppService;
use brrtrouter::{load_spec, router::Router};
use brrtrouter::server::HttpServer;
use clap::Parser;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value = "examples/openapi.yaml")]
    spec: PathBuf,
    #[arg(long)]
    static_dir: Option<PathBuf>,
}

fn parse_stack_size() -> usize {
    if let Ok(val) = std::env::var("BRRTR_STACK_SIZE") {
        if let Some(hex) = val.strip_prefix("0x") {
            usize::from_str_radix(hex, 16).unwrap_or(0x4000)
        } else {
            val.parse().unwrap_or(0x4000)
        }
    } else {
        0x4000
    }
}

fn main() -> io::Result<()> {
    // increase coroutine stack size to prevent overflows
    let args = Args::parse();
    let stack_size = parse_stack_size();
    may::config().set_stack_size(stack_size);
    // Load OpenAPI spec and create router
    let (routes, _slug) = load_spec(args.spec.to_str().unwrap()).expect("failed to load spec");
    let router = Router::new(routes.clone());

    // Create dispatcher and register handlers
    let mut dispatcher = Dispatcher::new();
    // unsafe {
    //     registry::register_all(&mut dispatcher);
    // }

    // Start the HTTP server on port 8080, binding to 127.0.0.1 if BRRTR_LOCAL is
    // set for local testing.
    // This returns a coroutine JoinHandle; we join on it to keep the server running
    let router = std::sync::Arc::new(std::sync::RwLock::new(Router::new(routes)));
    let dispatcher = std::sync::Arc::new(std::sync::RwLock::new(Dispatcher::new()));
    let service = AppService::new(
        router,
        dispatcher,
        HashMap::new(),
        args.spec.clone(),
        args.static_dir.clone(),
    );
    let addr = if std::env::var("BRRTR_LOCAL").is_ok() {
        "127.0.0.1:8080"
    } else {
        "0.0.0.0:8080"
    };
    println!("🚀 {{ name }} example server listening on {addr}");
    let server = HttpServer(service).start(addr).map_err(io::Error::other)?;
    println!("Server started successfully on {addr}");

    server
        .join()
        .map_err(|e| io::Error::other(format!("Server encountered an error: {:?}", e)))?;
    Ok(())
}
