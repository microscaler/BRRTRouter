// User-owned controller for handler 'get_label'.
use crate::handlers::get_label::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(GetLabelController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "color": "red"
    // }

    Response {
        color: Some("red".to_string()),
    }
}
