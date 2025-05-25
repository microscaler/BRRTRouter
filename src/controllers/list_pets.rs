
// User-owned controller for handler 'list_pets'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::list_pets::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // [
    //   {
    //     "age": 3,
    //     "breed": "Golden Retriever",
    //     "id": 12345,
    //     "name": "Max",
    //     "tags": [
    //       "friendly",
    //       "trained"
    //     ],
    //     "vaccinated": true
    //   },
    //   {
    //     "age": 2,
    //     "breed": "Labrador",
    //     "id": 67890,
    //     "name": "Bella",
    //     "tags": [
    //       "puppy",
    //       "playful"
    //     ],
    //     "vaccinated": true
    //   }
    // ]
    

    Response {
        items: vec![],
        
    }
}