
// User-owned controller for handler 'add_pet'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::add_pet::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
        
        id: Default::default(),
        
        name: Default::default(),
        
    }
}