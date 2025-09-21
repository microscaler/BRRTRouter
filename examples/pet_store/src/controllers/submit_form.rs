// User-owned controller for handler 'submit_form'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::submit_form::{Request, Response};
use brrtrouter_macros::handler;

#[handler(SubmitFormController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {}
}
