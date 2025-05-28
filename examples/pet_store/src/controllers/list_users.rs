
// User-owned controller for handler 'list_users'.
use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::list_users::{ Request, Response };
use crate::handlers::types::User;


pub struct ListUsersController;

impl Handler<Request, Response> for ListUsersController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        // Example response:
        // {
        //   "users": [
        //     {
        //       "id": "abc-123",
        //       "name": "John"
        //     },
        //     {
        //       "id": "def-456",
        //       "name": "Jane"
        //     }
        //   ]
        // }
        Response {
            users: Some(vec![serde_json::from_value::<User>(serde_json::json!({"id":"abc-123","name":"John"})).unwrap(), serde_json::from_value::<User>(serde_json::json!({"id":"def-456","name":"Jane"})).unwrap()]),
            
        }
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    ListUsersController.handle(req)
}
