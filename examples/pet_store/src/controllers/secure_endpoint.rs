// User-owned controller for handler 'secure_endpoint'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::secure_endpoint::{Request, Response};
use brrtrouter_macros::handler;

#[handler(SecureEndpointController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "status": "ok"
    // }

    Response {
        status: Some("ok".to_string()),
    }
}
