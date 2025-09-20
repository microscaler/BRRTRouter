// User-owned controller for handler 'list_users'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::list_users::{Request, Response};
use brrtrouter_macros::handler;

#[allow(unused_imports)]
use crate::handlers::types::User;

#[handler(ListUsersController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
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
        users: Some(vec![
            serde_json::from_value::<User>(serde_json::json!({"id":"abc-123","name":"John"}))
                .unwrap(),
            serde_json::from_value::<User>(serde_json::json!({"id":"def-456","name":"Jane"}))
                .unwrap(),
        ]),
    }
}
