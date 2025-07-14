// User-owned controller for handler 'get_user'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_user::{Request, Response};
use brrtrouter_macros::handler;

#[handler(GetUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "id": "abc-123",
    //   "name": "John"
    // }

    Response {
        id: Some("abc-123".to_string()),
        name: Some("John".to_string()),
    }
}
