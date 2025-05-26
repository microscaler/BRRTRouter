
// User-owned controller for handler 'get_user'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_user::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
    // 
    
    Response {
        
        id: "abc-123".to_string(),
        
        name: "John".to_string(),
        
    }
}