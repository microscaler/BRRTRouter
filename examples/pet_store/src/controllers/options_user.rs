// User-owned controller for handler 'options_user'.

use crate::handlers::options_user::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(OptionsUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "allow": "GET,HEAD,OPTIONS"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "allow": "GET,HEAD,OPTIONS"
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        allow: Some("GET,HEAD,OPTIONS".to_string()),
    }
}
