// User-owned controller for handler 'register_webhook'.

use crate::handlers::register_webhook::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(RegisterWebhookController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "subscription_id": "sub_123",
    //   "url": "https://example.com/webhook"
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "subscription_id": "sub_123",
  "url": "https://example.com/webhook"
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        subscription_id: Some("sub_123".to_string()),
        url: Some("https://example.com/webhook".to_string()),
    }
}
