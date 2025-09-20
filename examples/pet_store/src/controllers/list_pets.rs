// User-owned controller for handler 'list_pets'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::list_pets::{Request, Response};
use brrtrouter_macros::handler;

#[allow(unused_imports)]
use crate::handlers::types::Pet;

#[handler(ListPetsController)]
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

    Response(vec![serde_json::from_value::<Pet>(serde_json::json!({"age":3,"breed":"Golden Retriever","id":12345,"name":"Max","tags":["friendly","trained"],"vaccinated":true})).unwrap(), serde_json::from_value::<Pet>(serde_json::json!({"age":2,"breed":"Labrador","id":67890,"name":"Bella","tags":["puppy","playful"],"vaccinated":true})).unwrap()])
}
