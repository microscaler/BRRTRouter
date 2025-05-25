
// User-owned controller for handler 'add_pet'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::add_pet::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "id": 67890,
    //   "status": "success"
    // }
    

    Response {
        id: Some(42),
        status: Some("example".to_string()),
        
    }
}