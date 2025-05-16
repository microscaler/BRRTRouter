use brrrouter::server::AppService;
use brrrouter::{dispatcher::echo_handler, dispatcher::Dispatcher, load_spec, router::Router};
use may_minihttp::HttpServer;
use std::io;

fn main() -> io::Result<()> {
    // Load OpenAPI spec and create router
    let spec = load_spec("examples/openapi.yaml", false).expect("failed to load spec");
    let router = Router::new(spec);

    // Create the service instance
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("list_pets", echo_handler);
        dispatcher.register_handler("add_pet", echo_handler);
        dispatcher.register_handler("get_pet", echo_handler);
        dispatcher.register_handler("list_users", echo_handler);
        dispatcher.register_handler("get_user", echo_handler);
        dispatcher.register_handler("list_user_posts", echo_handler);
        dispatcher.register_handler("get_post", echo_handler);
        dispatcher.register_handler("admin_settings", echo_handler);
        dispatcher.register_handler("get_item", echo_handler);
        dispatcher.register_handler("post_item", echo_handler);
    }

    // Start the HTTP server on port 8080 (0.0.0.0:8080) under the may runtime
    // This returns a coroutine JoinHandle; we join on it to keep the server running
    let service = AppService { router, dispatcher };
    let server = HttpServer(service)
        .start("0.0.0.0:8080")
        .map_err(io::Error::other)?;
    println!("Server started successfully on 0.0.0.0:8080");
    server
        .join()
        .map_err(|e| io::Error::other(format!("Server encountered an error: {:?}", e)))?;
    Ok(())
}
