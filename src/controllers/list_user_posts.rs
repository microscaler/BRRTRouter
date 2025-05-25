// User-owned controller for handler 'list_user_posts'.

use crate::handlers::list_user_posts::{Request, Response};
use crate::typed::TypedHandlerRequest;

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response { items: vec![] }
}
