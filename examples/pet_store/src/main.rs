
use brrtrouter::{
    dispatcher::Dispatcher,
    router::Router,
    server::AppService,
};
use registry::register_all;
use may_minihttp::HttpServer;
use std::io;

fn main() -> io::Result<()> {
    let (routes, _slug) = brrtrouter::spec::load_spec("examples/pet_store/openapi.yaml")
        .expect("failed to load OpenAPI spec");

    let router = Router::new(routes);
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_all(&mut dispatcher);
    }

    let service = AppService { router, dispatcher };

    println!("🚀 pet_store example server listening on 0.0.0.0:8080");
    let server = HttpServer(service)
        .start("0.0.0.0:8080")
        .map_err(io::Error::other)?;

    server
        .join()
        .map_err(|e| io::Error::other(format!("Server failed: {:?}", e)))?;
    Ok(())
}