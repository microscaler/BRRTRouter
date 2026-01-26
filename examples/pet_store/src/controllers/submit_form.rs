
// User-owned controller for handler 'submit_form'.
use brrtrouter_macros::handler;
use brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::submit_form::{ Request, Response };



#[handler(SubmitFormController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
        // {
        //   "ok": true
        // }
    
    Response {
        ok: Some(true),
    }
    
    
}