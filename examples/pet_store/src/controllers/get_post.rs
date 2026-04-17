// User-owned controller for handler 'get_post'.

use crate::handlers::get_post::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(GetPostController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "author_id": "abc-123",
    //   "body": "Welcome to the blog",
    //   "id": "post1",
    //   "title": "Intro"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "author_id": "abc-123",
  "body": "Welcome to the blog",
  "id": "post1",
  "title": "Intro"
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        author_id: Some("abc-123".to_string()),
        body: "Welcome to the blog".to_string(),
        id: "post1".to_string(),
        title: "Intro".to_string(),
    }
}
