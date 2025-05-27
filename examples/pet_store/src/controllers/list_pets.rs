
// User-owned controller for handler 'list_pets'.

use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::list_pets::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
    // 
    
    Response {
        
        items: vec![Default::default()],
        
    }
}