// User-owned controller for handler 'secure_endpoint'.

use crate::handlers::secure_endpoint::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(SecureEndpointController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "status": "ok"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "status": "ok"
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        status: Some("ok".to_string()),
    }
}
