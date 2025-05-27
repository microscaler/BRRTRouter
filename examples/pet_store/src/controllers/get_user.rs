
// User-owned controller for handler 'get_user'.

use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_user::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
    // 
    
    Response {
        
        id: Some("abc-123".to_string()),
        
        name: Some("John".to_string()),
        
    }
}