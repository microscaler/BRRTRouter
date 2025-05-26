
// User-owned controller for handler 'add_pet'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::add_pet::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
    // 
    
    Response {
        
        id: 67890,
        
        status: "success".to_string(),
        
    }
}