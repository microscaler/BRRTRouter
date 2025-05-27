
// User-owned controller for handler 'get_post'.

use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_post::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
    // 
    
    Response {
        
        body: Some("Welcome to the blog".to_string()),
        
        id: Some("post1".to_string()),
        
        title: Some("Intro".to_string()),
        
    }
}