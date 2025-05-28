
// User-owned controller for handler 'get_pet'.

use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::get_pet::{ Request, Response };

pub struct GetPetController;

impl Handler<Request, Response> for GetPetController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        
        // Example response:
        // 
        
        Response {
            
            age: 3,
            
            breed: "Golden Retriever".to_string(),
            
            id: 12345,
            
            name: "Max".to_string(),
            
            tags: vec!["friendly".to_string().parse().unwrap(), "trained".to_string().parse().unwrap()],
            
            vaccinated: true,
            
        }
    }
}
pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    GetPetController.handle(req)
}

