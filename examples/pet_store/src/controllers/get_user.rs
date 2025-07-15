// User-owned controller for handler 'get_user'.
use crate::handlers::get_user::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter::{ValidationError, ValidationResult};
use brrtrouter_macros::handler;

use crate::handlers::types::UserPreferences;

#[handler(GetUserController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ValidationResult<Response> {
    // Example response:
    // {
    //   "id": "abc-123",
    //   "name": "John"
    // }

    Ok(Response {
        created_at: Some("2023-01-01T00:00:00Z".to_string()),
        email: "john@example.com".to_string(),
        id: "abc-123".to_string(),
        last_login: Some("2023-06-15T14:30:00Z".to_string()),
        name: "John".to_string(),
        phone: Some("+1-555-123-4567".to_string()),
        preferences: Some(
            serde_json::from_value::<UserPreferences>(
                serde_json::json!({"language":"en","timezone":"America/New_York"}),
            )
            .unwrap(),
        ),
        role: Some("user".to_string()),
        status: Some("active".to_string()),
    })
}
