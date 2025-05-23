
// User-owned controller for handler 'get_user'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_user::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
        
        id: Default::default(),
        
        name: Default::default(),
        
    }
}