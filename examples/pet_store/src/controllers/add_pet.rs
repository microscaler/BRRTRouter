
// User-owned controller for handler 'add_pet'.
use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::add_pet::{ Request, Response };


pub struct AddPetController;

impl Handler<Request, Response> for AddPetController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        // Example response:
        // {
        //   "id": 67890,
        //   "status": "success"
        // }
        Response {
            
            id: Some(67890),
            
            status: Some("success".to_string()),
            
        }
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    AddPetController.handle(req)
}