// User-owned controller for handler 'admin_settings'.

use crate::handlers::admin_settings::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(AdminSettingsController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "feature_flags": {
    //     "analytics": false,
    //     "beta": true
    //   }
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "feature_flags": {
    "analytics": false,
    "beta": true
  }
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        feature_flags: Some(serde_json::json!({"analytics":false,"beta":true})),
    }
}
