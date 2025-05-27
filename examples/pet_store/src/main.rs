use brrrouter::dispatcher::Dispatcher;
use brrrouter::router::Router;
use brrrouter::spec::RouteMeta;
use registry::register_all;
use std::sync::Arc;

fn main() {
    let mut dispatcher = Dispatcher::new();
    unsafe {
        register_all(&mut dispatcher);
    }

    let router = Router::new((
        vec![
            
            RouteMeta {
                method: Method::GET,
                path_pattern: "/admin/settings".to_string(),
                handler_name: "admin_settings".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
            RouteMeta {
                method: Method::GET,
                path_pattern: "/items/{id}".to_string(),
                handler_name: "get_item".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
            RouteMeta {
                method: Method::POST,
                path_pattern: "/items/{id}".to_string(),
                handler_name: "post_item".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
            RouteMeta {
                method: Method::GET,
                path_pattern: "/pets".to_string(),
                handler_name: "list_pets".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
            RouteMeta {
                method: Method::POST,
                path_pattern: "/pets".to_string(),
                handler_name: "add_pet".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
            RouteMeta {
                method: Method::GET,
                path_pattern: "/pets/{id}".to_string(),
                handler_name: "get_pet".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
            RouteMeta {
                method: Method::GET,
                path_pattern: "/users".to_string(),
                handler_name: "list_users".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
            RouteMeta {
                method: Method::GET,
                path_pattern: "/users/{user_id}".to_string(),
                handler_name: "get_user".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
            RouteMeta {
                method: Method::GET,
                path_pattern: "/users/{user_id}/posts".to_string(),
                handler_name: "list_user_posts".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
            RouteMeta {
                method: Method::GET,
                path_pattern: "/users/{user_id}/posts/{post_id}".to_string(),
                handler_name: "get_post".to_string(),
                parameters: vec![],
                request_schema: None,
                response_schema: None,
                example: None,
            },
            
        ],
        String::from(""),
    ));

    println!("Router initialized with 10 routes.");
}
