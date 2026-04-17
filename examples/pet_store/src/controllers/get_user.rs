// User-owned controller for handler 'get_user'.

use crate::handlers::get_user::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(GetUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "email": "john@example.com",
    //   "id": "abc-123",
    //   "name": "John"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "email": "john@example.com",
  "id": "abc-123",
  "name": "John"
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        email: Some("john@example.com".to_string()),
        id: "abc-123".to_string(),
        name: "John".to_string(),
    }
}
