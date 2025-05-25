
// User-owned controller for handler 'get_item'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_item::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
            id: Some("example".to_string()),
            name: Some("example".to_string()),}
}