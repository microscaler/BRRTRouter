
// User-owned controller for handler 'list_pets'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::list_pets::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
        
        age: Default::default(),
        
        breed: Default::default(),
        
        id: Default::default(),
        
        name: Default::default(),
        
        tags: Default::default(),
        
        vaccinated: Default::default(),
        
    }
}