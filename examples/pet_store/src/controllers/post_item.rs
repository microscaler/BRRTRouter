// User-owned controller for handler 'post_item'.

use crate::handlers::post_item::{ApiResponse, Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(PostItemController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ApiResponse {
    // Example response:
    // {
    //   "id": "item-001",
    //   "name": "New Item"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "id": "item-001",
  "name": "New Item"
}"###,
    ) {
        Ok(parsed) => return ApiResponse::Ok(parsed),
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    ApiResponse::Ok(Response {
        id: Some("item-001".to_string()),
        name: Some("New Item".to_string()),
    })
}
