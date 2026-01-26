
// User-owned controller for handler 'get_label'.
use brrtrouter_macros::handler;
use brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_label::{ Request, Response };



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