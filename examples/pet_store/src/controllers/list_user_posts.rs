// User-owned controller for handler 'list_user_posts'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::list_user_posts::{Request, Response};
use brrtrouter_macros::handler;

#[allow(unused_imports)]
use crate::handlers::types::Post;

#[handler(ListUserPostsController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // [
    //   {
    //     "author_id": "abc-123",
    //     "body": "Welcome to the blog",
    //     "id": "post1",
    //     "title": "Intro"
    //   },
    //   {
    //     "author_id": "abc-123",
    //     "body": "Thanks for reading",
    //     "id": "post2",
    //     "title": "Follow-up"
    //   }
    // ]

    Response(vec![
        match serde_json::from_value::<Post>(
            serde_json::json!({"author_id":"abc-123","body":"Welcome to the blog","id":"post1","title":"Intro"}),
        ) {
            Ok(v) => v,
            Err(_) => Default::default(),
        },
        match serde_json::from_value::<Post>(
            serde_json::json!({"author_id":"abc-123","body":"Thanks for reading","id":"post2","title":"Follow-up"}),
        ) {
            Ok(v) => v,
            Err(_) => Default::default(),
        },
    ])
}
