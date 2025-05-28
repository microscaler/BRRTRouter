
use brrtrouter::{
    dispatcher::Dispatcher,
    router::Router,
    server::AppService,
};
use pet_store::registry;
use may_minihttp::HttpServer;
use std::io;

fn main() -> io::Result<()> {
    let (routes, _slug) = brrtrouter::spec::load_spec("./openapi.yaml")
        .expect("failed to load OpenAPI spec");

    let router = Router::new(routes);
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_all(&mut dispatcher);
    }

    let service = AppService { router, dispatcher };
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
        .map_err(|e| io::Error::other(format!("Server failed: {:?}", e)))?;
    Ok(())
}
