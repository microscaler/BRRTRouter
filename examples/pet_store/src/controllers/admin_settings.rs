
// User-owned controller for handler 'admin_settings'.
use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::admin_settings::{ Request, Response };


pub struct AdminSettingsController;

impl Handler<Request, Response> for AdminSettingsController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
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
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    AdminSettingsController.handle(req)
}