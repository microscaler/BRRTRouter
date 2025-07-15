// User-owned controller for handler 'add_pet'.
use crate::handlers::add_pet::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter::{ValidationError, ValidationResult};
use brrtrouter_macros::handler;

#[handler(AddPetController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ValidationResult<Response> {
    Ok(Response {})
}
