
// User-owned controller for handler 'get_user'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_user::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
            id: Some("example".to_string()),
            name: Some("example".to_string()),}
}