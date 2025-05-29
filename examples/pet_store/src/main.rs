
use brrtrouter::{
    dispatcher::Dispatcher,
    middleware::{AuthMiddleware, CorsMiddleware, MetricsMiddleware, TracingMiddleware},
    router::Router,
    server::AppService,
};
use std::collections::HashMap;
use pet_store::registry;
use may_minihttp::HttpServer;
use std::io;

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
    // enlarge stack size for may coroutines
    let stack_size = parse_stack_size();
    may::config().set_stack_size(stack_size);
    // Load OpenAPI spec and create router
    let (routes, _slug) = brrtrouter::spec::load_spec("./openapi.yaml").expect("failed to load OpenAPI spec");
    let router = Router::new(routes.clone());

    // Create dispatcher, register handlers, and stack middlewares
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let metrics = std::sync::Arc::new(MetricsMiddleware::new());
    let tracing_mw = std::sync::Arc::new(TracingMiddleware);
    let auth = std::sync::Arc::new(AuthMiddleware::new("Bearer secret".into()));
    let cors = std::sync::Arc::new(CorsMiddleware);
    dispatcher.add_middleware(metrics);
    dispatcher.add_middleware(tracing_mw);
    dispatcher.add_middleware(auth);
    dispatcher.add_middleware(cors);

    // Start the HTTP server on port 8080, binding to 127.0.0.1 if BRRTR_LOCAL is
    // set for local testing.
    // This returns a coroutine JoinHandle; we join on it to keep the server running
    let router = std::sync::Arc::new(std::sync::RwLock::new(router));
    let dispatcher = std::sync::Arc::new(std::sync::RwLock::new(dispatcher));
    let service = AppService::new(router, dispatcher, HashMap::new());
    let addr = if std::env::var("BRRTR_LOCAL").is_ok() {
        "127.0.0.1:8080"
    } else {
        "0.0.0.0:8080"
    };
    println!("🚀 pet_store example server listening on {addr}");
    let server = HttpServer(service)
        .start(addr)
        .map_err(io::Error::other)?;
    println!("Server started successfully on {addr}");

    server
        .join()
        .map_err(|e| io::Error::other(format!("Server encountered an error: {:?}", e)))?;
    Ok(())
}
