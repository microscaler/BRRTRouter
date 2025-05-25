// User-owned controller for handler 'get_item'.

use crate::handlers::get_item::{Request, Response};
use crate::typed::TypedHandlerRequest;

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
        id: Some("example".to_string()),

        name: Some("example".to_string()),
    }
}
