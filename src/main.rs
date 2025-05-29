use brrtrouter::dispatcher::Dispatcher;
// use brrrouter::registry;
use brrtrouter::server::AppService;
use brrtrouter::{load_spec, router::Router};
use std::collections::HashMap;
use may_minihttp::HttpServer;
use std::io;

fn main() -> io::Result<()> {
    // enlarge stack size for may coroutines
    may::config().set_stack_size(0x8000);
    // Load OpenAPI spec and create router
    let (routes, _slug) = load_spec("examples/openapi.yaml").expect("failed to load spec");
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
    let service = AppService::new(router, dispatcher, HashMap::new());
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
