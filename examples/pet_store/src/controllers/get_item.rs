// User-owned controller for handler 'get_item'.

use crate::handlers::get_item::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(GetItemController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "id": "item-001",
    //   "name": "Sample Item"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "id": "item-001",
  "name": "Sample Item"
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        id: Some("item-001".to_string()),
        name: Some("Sample Item".to_string()),
    }
}
