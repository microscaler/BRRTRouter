
// User-owned controller for handler 'stream_events'.
use brrtrouter_macros::handler;
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::stream_events::{ Request, Response };
use crate::brrtrouter::sse;


#[handler(StreamEventsController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    let (tx, rx) = sse::channel();
    for i in 0..3 { tx.send(format!("tick {i}")); }
    drop(tx);
    Response(rx.collect())
    
}