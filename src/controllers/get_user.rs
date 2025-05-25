
// User-owned controller for handler 'get_user'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_user::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "id": "abc-123",
    //   "name": "John"
    // }
    

    Response {
        id: Some("example".to_string()),
        name: Some("example".to_string()),
        
    }
}