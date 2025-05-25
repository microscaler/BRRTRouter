// User-owned controller for handler 'get_post'.

use crate::handlers::get_post::{Request, Response};
use crate::typed::TypedHandlerRequest;

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
        body: Some("example".to_string()),

        id: Some("example".to_string()),

        title: Some("example".to_string()),
    }
}
