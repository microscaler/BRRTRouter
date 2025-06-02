
// User-owned controller for handler 'add_pet'.
use brrtrouter_macros::handler;
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::add_pet::{ Request, Response };



#[handler(AddPetController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
        // {
        //   "id": 67890,
        //   "status": "success"
        // }
    
    Response {
        id: Some(67890),
        status: Some("success".to_string()),
        
    }
    
    
}
