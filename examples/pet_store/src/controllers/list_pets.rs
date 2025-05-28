
// User-owned controller for handler 'list_pets'.

use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::list_pets::{ Request, Response };

pub struct ListPetsController;

impl Handler<Request, Response> for ListPetsController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        
        // Example response:
        // 
        
        Response {
            
            items: vec![Default::default()],
            
        }
    }
}