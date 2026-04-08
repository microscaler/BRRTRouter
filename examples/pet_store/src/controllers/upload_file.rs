// User-owned controller for handler 'upload_file'.

use crate::handlers::upload_file::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(UploadFileController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "location": "https://cdn.example.com/files/abc.png"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "location": "https://cdn.example.com/files/abc.png"
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        location: Some("https://cdn.example.com/files/abc.png".to_string()),
    }
}
