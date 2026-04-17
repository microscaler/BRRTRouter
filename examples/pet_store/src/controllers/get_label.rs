// User-owned controller for handler 'get_label'.

use crate::handlers::get_label::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(GetLabelController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "color": "red"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "color": "red"
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        color: Some("red".to_string()),
    }
}
