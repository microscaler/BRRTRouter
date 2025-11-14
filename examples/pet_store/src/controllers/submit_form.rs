// User-owned controller for handler 'submit_form'.
use crate::handlers::submit_form::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(SubmitFormController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "ok": true
    // }

    Response { ok: Some(true) }
}
