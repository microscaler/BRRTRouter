// User-owned controller for handler 'list_users'.
use crate::handlers::list_users::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter::{ValidationError, ValidationResult};
use brrtrouter_macros::handler;

use crate::handlers::types::User;

#[handler(ListUsersController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ValidationResult<Response> {
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

    Ok(Response {
        page: 1,
        per_page: 10,
        total: 150,
        total_pages: Some(15),
        users: vec![
            serde_json::from_value::<User>(serde_json::json!({"id":"abc-123","name":"John"}))
                .unwrap(),
            serde_json::from_value::<User>(serde_json::json!({"id":"def-456","name":"Jane"}))
                .unwrap(),
        ],
    })
}
