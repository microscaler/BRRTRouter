
// User-owned controller for handler 'get_post'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_post::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
    // 
    
    Response {
        
        body: "Welcome to the blog".to_string(),
        
        id: "post1".to_string(),
        
        title: "Intro".to_string(),
        
    }
}