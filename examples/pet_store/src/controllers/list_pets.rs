
// User-owned controller for handler 'list_pets'.
use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::list_pets::{ Request, Response };
use crate::handlers::types::Pet;


pub struct ListPetsController;

impl Handler<Request, Response> for ListPetsController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
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
            items: vec![Default::default()],
            
        }
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    ListPetsController.handle(req)
}
