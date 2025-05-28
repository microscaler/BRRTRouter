use brrtrouter::dispatcher::Dispatcher;
// use brrrouter::registry;
use brrtrouter::server::AppService;
use brrtrouter::{load_spec, router::Router};
use may_minihttp::HttpServer;
use std::io;

fn main() -> io::Result<()> {
    // Load OpenAPI spec and create router
    let (routes, _slug) = load_spec("examples/openapi.yaml", false).expect("failed to load spec");
    let router = Router::new(routes);

    // Create dispatcher and register handlers
    let dispatcher = Dispatcher::new();
    // unsafe {
    //     registry::register_all(&mut dispatcher);
    // }

    // Start the HTTP server on port 8080. If the `BRRTR_LOCAL` environment
    // variable is set, bind to 127.0.0.1 for easier local testing.
    let bind_addr = if std::env::var("BRRTR_LOCAL").is_ok() {
        "127.0.0.1:8080"
    } else {
        "0.0.0.0:8080"
    };

    // This returns a coroutine JoinHandle; we join on it to keep the server running
    let service = AppService { router, dispatcher };
    let server = HttpServer(service)
        .start(bind_addr)
        .map_err(io::Error::other)?;
    println!("Server started successfully on {}", bind_addr);
    server
        .join()
        .map_err(|e| io::Error::other(format!("Server encountered an error: {:?}", e)))?;
    Ok(())
}
