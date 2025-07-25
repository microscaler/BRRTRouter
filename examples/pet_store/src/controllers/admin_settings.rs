// User-owned controller for handler 'admin_settings'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::admin_settings::{Request, Response};
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

    Response {
        feature_flags: Some(serde_json::json!({"analytics":false,"beta":true})),
    }
}
