use serde::{ Deserialize, Serialize };
use crate::brrtrouter::typed::TypedHandlerRequest;

#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    pub id: String,
    }

#[derive(Debug, Serialize)]
pub struct Response {
    pub age: i32,
    pub breed: String,
    pub id: i32,
    pub name: String,
    pub tags: Vec<serde_json::Value>,
    pub vaccinated: bool,
    }

pub fn handler(req: TypedHandlerRequest<Request>) -> Response {
    crate::controllers::get_pet::handle(req)
}
