
// User-owned controller for handler 'list_users'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::list_users::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
    // 
    
    Response {
        
        users: Some(vec![Default::default(), Default::default()]),
        
    }
}