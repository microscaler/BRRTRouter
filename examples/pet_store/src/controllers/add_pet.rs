// User-owned controller for handler 'add_pet'.

use crate::handlers::add_pet::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(AddPetController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "id": 67890,
    //   "status": "success"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "id": 67890,
  "status": "success"
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        id: Some(67890),
        status: Some("success".to_string()),
    }
}
