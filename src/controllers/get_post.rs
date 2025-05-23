
// User-owned controller for handler 'get_post'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_post::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
        
        body: Default::default(),
        
        id: Default::default(),
        
        title: Default::default(),
        
    }
}