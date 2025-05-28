use serde::{ Deserialize, Serialize };
use crate::brrtrouter::typed::TypedHandlerRequest;

use crate::handlers::types::Pet;
#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    }

#[derive(Debug, Serialize)]
pub struct Response {
    pub items: Vec<Pet>,
    }

pub fn handler(req: TypedHandlerRequest<Request>) -> Response {
    crate::controllers::list_pets::handle(req)
}
