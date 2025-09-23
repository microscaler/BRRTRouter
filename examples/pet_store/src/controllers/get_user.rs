// User-owned controller for handler 'get_user'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_user::{Request, Response};
use brrtrouter_macros::handler;

#[handler(GetUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "email": "john@example.com",
    //   "id": "abc-123",
    //   "name": "John"
    // }

    Response {
        email: Some("john@example.com".to_string()),
        id: "abc-123".to_string(),
        name: "John".to_string(),
    }
}
