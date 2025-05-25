
// User-owned controller for handler 'post_item'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::post_item::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
            id: Some("example".to_string()),
            name: Some("example".to_string()),}
}