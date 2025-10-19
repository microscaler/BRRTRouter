
// User-owned controller for handler 'register_webhook'.
use brrtrouter_macros::handler;
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::register_webhook::{ Request, Response };



#[handler(RegisterWebhookController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
        // {
        //   "subscription_id": "sub_123",
        //   "url": "https://example.com/webhook"
        // }
    
    Response {
        subscription_id: Some("sub_123".to_string()),
        url: Some("https://example.com/webhook".to_string()),
        
    }
    
    
}