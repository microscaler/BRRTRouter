
// User-owned controller for handler 'get_matrix'.
use brrtrouter_macros::handler;
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_matrix::{ Request, Response };



#[handler(GetMatrixController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
        // {
        //   "coords": [
        //     1,
        //     2,
        //     3
        //   ]
        // }
    
    Response {
        coords: Some(vec![1, 2, 3]),
        
    }
    
    
}