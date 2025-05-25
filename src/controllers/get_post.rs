
// User-owned controller for handler 'get_post'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_post::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "body": "Welcome to the blog",
    //   "id": "post1",
    //   "title": "Intro"
    // }
    

    Response {
        body: Some("example".to_string()),
        id: Some("example".to_string()),
        title: Some("example".to_string()),
        
    }
}