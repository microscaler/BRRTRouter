// User-owned controller for handler 'delete_user'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::delete_user::{Request, Response};
use brrtrouter_macros::handler;

#[handler(DeleteUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {}
}
