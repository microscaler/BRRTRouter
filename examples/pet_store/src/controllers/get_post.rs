
// User-owned controller for handler 'get_post'.
use brrtrouter_macros::handler;
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_post::{ Request, Response };



#[handler(GetPostController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
        // {
        //   "body": "Welcome to the blog",
        //   "id": "post1",
        //   "title": "Intro"
        // }
    
    Response {
        body: Some("Welcome to the blog".to_string()),
        id: Some("post1".to_string()),
        title: Some("Intro".to_string()),
        
    }
    
    
}
