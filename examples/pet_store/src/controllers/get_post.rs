// User-owned controller for handler 'get_post'.
use crate::handlers::get_post::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter::{ValidationError, ValidationResult};
use brrtrouter_macros::handler;

#[handler(GetPostController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ValidationResult<Response> {
    // Example response:
    // {
    //   "body": "Welcome to the blog",
    //   "id": "post1",
    //   "title": "Intro"
    // }

    Ok(Response {
        author_id: "user-123".to_string(),
        body: "Welcome to the blog".to_string(),
        created_at: Some("2023-01-15T10:30:00Z".to_string()),
        id: "post1".to_string(),
        metadata: Some(
            serde_json::json!({"seo_description":"An introduction to our blog","seo_title":"Welcome to Our Blog"}),
        ),
        published_at: Some("2023-01-15T12:00:00Z".to_string()),
        status: Some("published".to_string()),
        tags: Some(vec!["introduction".to_string(), "welcome".to_string()]),
        title: "Intro".to_string(),
        updated_at: Some("2023-01-15T10:30:00Z".to_string()),
        view_count: Some(125),
    })
}
