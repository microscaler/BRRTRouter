// User-owned controller for handler 'delete_user'.
use crate::handlers::delete_user::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(DeleteUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {}
}
