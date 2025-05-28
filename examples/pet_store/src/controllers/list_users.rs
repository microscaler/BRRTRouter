
// User-owned controller for handler 'list_users'.

use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::list_users::{ Request, Response };

pub struct ListUsersController;

impl Handler<Request, Response> for ListUsersController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        
        // Example response:
        // 
        
        Response {
            
            users: Some(vec![Default::default(), Default::default()]),
            
        }
    }
}
pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    ListUsersController.handle(req)
}

