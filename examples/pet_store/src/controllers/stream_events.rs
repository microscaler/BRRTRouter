use crate::brrtrouter::sse;
use crate::brrtrouter::dispatcher::{HandlerRequest, HandlerResponse};
use may::coroutine;
use may::coroutine::Builder;
use std::time::Duration;

pub fn handle(req: HandlerRequest) {
    let (tx, rx) = sse::channel();
    // spawn a coroutine to emit periodic events
    unsafe {
        Builder::new()
            .name("stream_events_emitter".to_string())
            .stack_size(0x8001)
            .spawn(move || {
                for i in 0..3 {
                    tx.send(format!("tick {i}"));
                    may::coroutine::sleep(Duration::from_millis(50));
                }
            })
            .expect("spawn emitter");
    }
    let body = rx.collect();
    let resp = HandlerResponse { status: 200, body: serde_json::Value::String(body) };
    let _ = req.reply_tx.send(resp);
}
