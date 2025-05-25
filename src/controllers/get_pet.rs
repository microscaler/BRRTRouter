// User-owned controller for handler 'get_pet'.

use crate::handlers::get_pet::{Request, Response};
use crate::typed::TypedHandlerRequest;

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
        age: 42,

        breed: "example".to_string(),

        id: 42,

        name: "example".to_string(),

        tags: Default::default(),

        vaccinated: true,
    }
}
