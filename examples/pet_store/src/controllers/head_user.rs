
// User-owned controller for handler 'head_user'.
use brrtrouter_macros::handler;
use brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::head_user::{ Request, Response };



#[handler(HeadUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
        // {
        //   "exists": true
        // }
    
    Response {
        exists: Some(true),
    }
    
    
}