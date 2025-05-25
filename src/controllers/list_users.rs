
// User-owned controller for handler 'list_users'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::list_users::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "users": [
    //     {
    //       "id": "abc-123",
    //       "name": "John"
    //     },
    //     {
    //       "id": "def-456",
    //       "name": "Jane"
    //     }
    //   ]
    // }
    

    Response {
        users: Some(Default::default()),
        
    }
}