// User-owned controller for handler 'secure_endpoint'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::secure_endpoint::{Request, Response};
use brrtrouter_macros::handler;

#[handler(SecureEndpointController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {}
}
