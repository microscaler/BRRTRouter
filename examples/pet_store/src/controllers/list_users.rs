// User-owned controller for handler 'list_users'.
use crate::handlers::list_users::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[allow(unused_imports)]
use crate::handlers::types::User;

#[handler(ListUsersController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "users": [
    //     {
    //       "email": "john@example.com",
    //       "id": "abc-123",
    //       "name": "John"
    //     },
    //     {
    //       "email": "jane@example.com",
    //       "id": "def-456",
    //       "name": "Jane"
    //     }
    //   ]
    // }

    Response {
        users: Some(vec![
            serde_json::from_value::<User>(
                serde_json::json!({"email":"john@example.com","id":"abc-123","name":"John"}),
            )
            .unwrap_or_default(),
            serde_json::from_value::<User>(
                serde_json::json!({"email":"jane@example.com","id":"def-456","name":"Jane"}),
            )
            .unwrap_or_default(),
        ]),
    }
}
