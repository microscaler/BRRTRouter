// User-owned controller for handler 'head_user'.
use crate::handlers::head_user::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(HeadUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "exists": true
    // }

    Response { exists: Some(true) }
}
