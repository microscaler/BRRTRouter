
// User-owned controller for handler 'list_pets'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::list_pets::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
            items: vec![],
            }
}