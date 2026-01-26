// User-owned controller for handler 'options_user'.
use crate::handlers::options_user::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(OptionsUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "allow": "GET,HEAD,OPTIONS"
    // }

    Response {
        allow: Some("GET,HEAD,OPTIONS".to_string()),
    }
}
