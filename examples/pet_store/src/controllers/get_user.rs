
// User-owned controller for handler 'get_user'.

use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::get_user::{ Request, Response };

pub struct GetUserController;

impl Handler<Request, Response> for GetUserController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        
        // Example response:
        // 
        
        Response {
            
            id: Some("abc-123".to_string()),
            
            name: Some("John".to_string()),
            
        }
    }
}
pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    GetUserController.handle(req)
}

