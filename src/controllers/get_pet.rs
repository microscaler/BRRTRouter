
// User-owned controller for handler 'get_pet'.

use crate::typed::TypedHandlerRequest;
use crate::handlers::get_pet::{ Request, Response };

pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
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