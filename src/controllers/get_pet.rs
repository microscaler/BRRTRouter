
// User-owned controller for handler 'get_pet'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_pet::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "age": 3,
    //   "breed": "Golden Retriever",
    //   "id": 12345,
    //   "name": "Max",
    //   "tags": [
    //     "friendly",
    //     "trained"
    //   ],
    //   "vaccinated": true
    // }
    

    Response {
        age: 42,
        breed: "example".to_string(),
        id: 42,
        name: "example".to_string(),
        tags: Default::default(),
        vaccinated: true,
        
    }
}