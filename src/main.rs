use brrrouter::{spec::load_spec, router::Router};
use http::Method;

fn main() -> anyhow::Result<()> {
    let verbose = std::env::args().any(|arg| arg == "--verbose" || arg == "-v");

    let routes = load_spec("examples/openapi.yaml", verbose)?;
    let router = Router::new(routes);

    let tests = vec![
        (Method::GET, "/pets", "list_pets"),
        (Method::POST, "/pets", "add_pet"),
        (Method::GET, "/pets/42", "get_pet"),
        (Method::GET, "/users", "list_users"),
        (Method::GET, "/users/99", "get_user"),
        (Method::GET, "/users/99/posts", "list_user_posts"),
        (Method::GET, "/users/99/posts/abc", "get_post"),
        (Method::GET, "/admin/settings", "admin_settings"),
        (Method::GET, "/does/not/exist", "<none>"),
    ];

    for (method, path, expected) in tests {
        let result = router.route(method.clone(), path);
        match result {
            Some(matched) => {
                println!("✅ {} {} → handler: {} | params: {:?}", method, path, matched.route.handler_name, matched.path_params);
                assert_eq!(matched.route.handler_name, expected);
            }
            None => {
                println!("❌ {} {} → no match (expected: {})", method, path, expected);
                assert_eq!(expected, "<none>");
            }
        }
    }

    Ok(())
}
