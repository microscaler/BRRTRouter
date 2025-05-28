// User-owned controller for handler 'get_pet'.
use crate::brrtrouter::typed::{Handler, TypedHandlerRequest};
use crate::handlers::get_pet::{Request, Response};

pub struct GetPetController;

impl Handler<Request, Response> for GetPetController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
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
            age: 3,
            breed: "Golden Retriever".to_string(),
            id: 12345,
            name: "Max".to_string(),
            tags: vec!["friendly".to_string(), "trained".to_string()],
            vaccinated: true,
        }
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    GetPetController.handle(req)
}
