
// User-owned controller for handler 'get_post'.

use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::get_post::{ Request, Response };

pub struct GetPostController;

impl Handler<Request, Response> for GetPostController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        
        // Example response:
        // 
        
        Response {
            
            body: Some("Welcome to the blog".to_string()),
            
            id: Some("post1".to_string()),
            
            title: Some("Intro".to_string()),
            
        }
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    GetPostController.handle(req)
}