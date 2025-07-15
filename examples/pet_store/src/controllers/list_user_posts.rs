// User-owned controller for handler 'list_user_posts'.
use crate::handlers::list_user_posts::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter::{ValidationError, ValidationResult};
use brrtrouter_macros::handler;

use crate::handlers::types::Post;

#[handler(ListUserPostsController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ValidationResult<Response> {
    // Example response:
    // [
    //   {
    //     "body": "Welcome to the blog",
    //     "id": "post1",
    //     "title": "Intro"
    //   },
    //   {
    //     "body": "Thanks for reading",
    //     "id": "post2",
    //     "title": "Follow-up"
    //   }
    // ]

    Ok(Response(vec![serde_json::from_value::<Post>(serde_json::json!({"author_id":"user-123","body":"Welcome to the blog","created_at":"2023-01-15T10:30:00Z","id":"post1","metadata":{"seo_description":"An introduction to our blog","seo_title":"Welcome to Our Blog"},"published_at":"2023-01-15T12:00:00Z","status":"published","tags":["introduction","welcome"],"title":"Intro","updated_at":"2023-01-15T10:30:00Z","view_count":125})).unwrap(), serde_json::from_value::<Post>(serde_json::json!({"author_id":"user-123","body":"Welcome to the blog","created_at":"2023-01-15T10:30:00Z","id":"post1","metadata":{"seo_description":"An introduction to our blog","seo_title":"Welcome to Our Blog"},"published_at":"2023-01-15T12:00:00Z","status":"published","tags":["introduction","welcome"],"title":"Intro","updated_at":"2023-01-15T10:30:00Z","view_count":125})).unwrap()]))
}
