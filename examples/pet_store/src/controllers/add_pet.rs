
// User-owned controller for handler 'add_pet'.

use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::add_pet::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
    // 
    
    Response {
        
        id: Some(67890),
        
        status: Some("success".to_string()),
        
    }
}