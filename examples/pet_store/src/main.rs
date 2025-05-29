use brrtrouter::{dispatcher::Dispatcher, router::Router, server::AppService, hot_reload};
use may_minihttp::HttpServer;
use pet_store::registry;
use std::io;
use std::sync::{Arc, RwLock};

fn main() -> io::Result<()> {
    let (routes, _slug) =
        brrtrouter::spec::load_spec("./openapi.yaml").expect("failed to load OpenAPI spec");

    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));
    unsafe {
        registry::register_from_spec(&mut dispatcher.write().unwrap(), &routes);
    }
    // Watch the spec file and reload routes on changes
    let _watcher = hot_reload::watch_spec(
        "./openapi.yaml",
        Arc::clone(&router),
        {
            let dispatcher = Arc::clone(&dispatcher);
            move |routes| {
                let mut d = dispatcher.write().unwrap();
                d.handlers.clear();
                unsafe { registry::register_from_spec(&mut d, &routes) };
            }
        },
    )
    .expect("failed to watch spec");

    let service = AppService { router, dispatcher }; 
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
        .map_err(|e| io::Error::other(format!("Server failed: {:?}", e)))?;
    Ok(())
}
