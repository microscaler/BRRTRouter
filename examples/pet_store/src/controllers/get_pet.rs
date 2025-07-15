// User-owned controller for handler 'get_pet'.
use crate::handlers::get_pet::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter::{ValidationError, ValidationResult};
use brrtrouter_macros::handler;

use crate::handlers::types::MedicalRecord;
use crate::handlers::types::PetOwner;
use crate::handlers::types::Photo;

#[handler(GetPetController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ValidationResult<Response> {
    // Example response:
    // {
    //   "age": 3,
    //   "breed": "Golden Retriever",
    //   "id": 12345,
    //   "medical_records": [
    //     {
    //       "date": "2023-01-15",
    //       "description": "Annual vaccination",
    //       "record_type": "vaccination",
    //       "veterinarian": "Dr. Johnson"
    //     }
    //   ],
    //   "name": "Max",
    //   "owner": {
    //     "email": "john@example.com",
    //     "id": "user-123",
    //     "name": "John Smith"
    //   },
    //   "tags": [
    //     "friendly",
    //     "trained"
    //   ],
    //   "vaccinated": true
    // }

    Ok(Response {
        age: 3,
        breed: "Golden Retriever".to_string(),
        created_at: Some("2023-01-15T10:30:00Z".to_string()),
        id: 12345,
        medical_records: Some(vec![serde_json::from_value::<MedicalRecord>(serde_json::json!({"date":"2023-01-15","description":"Annual vaccination","record_type":"vaccination","veterinarian":"Dr. Johnson"})).unwrap()]),
        name: "Max".to_string(),
        owner: Some(serde_json::from_value::<PetOwner>(serde_json::json!({"email":"john@example.com","id":"user-123","name":"John Smith","phone":"+1-555-123-4567"})).unwrap()),
        photos: Some(Default::default()),
        status: Some("available".to_string()),
        tags: vec!["friendly".to_string(), "trained".to_string()],
        updated_at: Some("2023-06-10T14:45:00Z".to_string()),
        vaccinated: true,
        weight: Some(25.5),
        })
}
