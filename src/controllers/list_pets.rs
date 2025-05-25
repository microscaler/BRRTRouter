// User-owned controller for handler 'list_pets'.

use crate::handlers::list_pets::{Request, Response};
use crate::typed::TypedHandlerRequest;

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response { items: vec![] }
}
