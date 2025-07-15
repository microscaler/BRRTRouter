// User-owned controller for handler 'list_pets'.
use crate::handlers::list_pets::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter::{ValidationError, ValidationResult};
use brrtrouter_macros::handler;

use crate::handlers::types::Pet;

#[handler(ListPetsController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ValidationResult<Response> {
    // Example response:
    // [
    //   {
    //     "age": 3,
    //     "breed": "Golden Retriever",
    //     "id": 12345,
    //     "medical_records": [
    //       {
    //         "date": "2023-01-15",
    //         "description": "Annual vaccination",
    //         "record_type": "vaccination",
    //         "veterinarian": "Dr. Johnson"
    //       },
    //       {
    //         "date": "2023-06-10",
    //         "description": "Regular health checkup",
    //         "record_type": "checkup",
    //         "veterinarian": "Dr. Johnson"
    //       }
    //     ],
    //     "name": "Max",
    //     "owner": {
    //       "email": "john@example.com",
    //       "id": "user-123",
    //       "name": "John Smith"
    //     },
    //     "tags": [
    //       "friendly",
    //       "trained"
    //     ],
    //     "vaccinated": true
    //   },
    //   {
    //     "age": 2,
    //     "breed": "Labrador",
    //     "id": 67890,
    //     "medical_records": [
    //       {
    //         "date": "2023-03-20",
    //         "description": "Puppy vaccination series",
    //         "record_type": "vaccination",
    //         "veterinarian": "Dr. Smith"
    //       }
    //     ],
    //     "name": "Bella",
    //     "owner": {
    //       "email": "jane@example.com",
    //       "id": "user-456",
    //       "name": "Jane Doe"
    //     },
    //     "tags": [
    //       "puppy",
    //       "playful"
    //     ],
    //     "vaccinated": true
    //   }
    // ]

    Ok(Response(vec![serde_json::from_value::<Pet>(serde_json::json!({"age":3,"breed":"Golden Retriever","created_at":"2023-01-15T10:30:00Z","id":12345,"name":"Max","status":"available","tags":["friendly","trained"],"updated_at":"2023-06-10T14:45:00Z","vaccinated":true,"weight":25.5})).unwrap(), serde_json::from_value::<Pet>(serde_json::json!({"age":3,"breed":"Golden Retriever","created_at":"2023-01-15T10:30:00Z","id":12345,"name":"Max","status":"available","tags":["friendly","trained"],"updated_at":"2023-06-10T14:45:00Z","vaccinated":true,"weight":25.5})).unwrap()]))
}
