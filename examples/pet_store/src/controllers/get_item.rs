// User-owned controller for handler 'get_item'.
use crate::handlers::get_item::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter::{ValidationError, ValidationResult};
use brrtrouter_macros::handler;

#[handler(GetItemController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ValidationResult<Response> {
    // Example response:
    // {
    //   "id": "item-001",
    //   "name": "Sample Item"
    // }

    Ok(Response {
        category: "toy".to_string(),
        created_at: Some("2023-01-15T10:30:00Z".to_string()),
        currency: Some("USD".to_string()),
        description: Some("A fun toy for pets".to_string()),
        id: "item-001".to_string(),
        in_stock: Some(true),
        name: "Sample Item".to_string(),
        price: Some(19.99),
        stock_quantity: Some(50),
    })
}
