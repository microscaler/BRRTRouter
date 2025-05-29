use brrtrouter::{dispatcher::Dispatcher, router::Router, server::AppService};
use may_minihttp::HttpServer;
use pet_store::registry;
use std::collections::HashMap;
use std::io;

fn main() -> io::Result<()> {
    // enlarge stack size for may coroutines
    may::config().set_stack_size(0x8000);
    // Load OpenAPI spec and create router
    let (routes, _slug) =
        brrtrouter::spec::load_spec("./openapi.yaml").expect("failed to load OpenAPI spec");
    let router = Router::new(routes.clone());

    // Create dispatcher and register handlers
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }

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
    println!("🚀 pet_store example server listening on {addr}");
    let server = HttpServer(service).start(addr).map_err(io::Error::other)?;
    println!("Server started successfully on {addr}");

    server
        .join()
        .map_err(|e| io::Error::other(format!("Server encountered an error: {:?}", e)))?;
    Ok(())
}
