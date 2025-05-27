
// User-owned controller for handler 'list_user_posts'.

use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::list_user_posts::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
    // 
    
    Response {
        
        items: vec![Default::default()],
        
    }
}