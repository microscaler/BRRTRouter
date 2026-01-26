
// User-owned controller for handler 'get_post'.
use brrtrouter_macros::handler;
use brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_post::{ Request, Response };



#[handler(GetPostController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
        // {
        //   "author_id": "abc-123",
        //   "body": "Welcome to the blog",
        //   "id": "post1",
        //   "title": "Intro"
        // }
    
    Response {
        author_id: Some("abc-123".to_string()),body: "Welcome to the blog".to_string(),id: "post1".to_string(),title: "Intro".to_string(),
    }
    
    
}