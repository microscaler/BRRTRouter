// User-owned controller for handler 'get_label'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_label::{Request, Response};
use brrtrouter_macros::handler;

#[handler(GetLabelController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {}
}
