// User-owned controller for handler 'post_item'.
use crate::handlers::post_item::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter::{ValidationError, ValidationResult};
use brrtrouter_macros::handler;

#[handler(PostItemController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> ValidationResult<Response> {
    // Example response:
    // {
    //   "id": "item-001",
    //   "name": "New Item"
    // }

    Ok(Response {
        category: "toy".to_string(),
        created_at: Some("2023-01-15T10:30:00Z".to_string()),
        currency: Some("USD".to_string()),
        description: Some("A fun toy for pets".to_string()),
        id: "item-001".to_string(),
        in_stock: Some(true),
        name: "New Item".to_string(),
        price: Some(19.99),
        stock_quantity: Some(50),
    })
}
