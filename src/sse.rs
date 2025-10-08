//! # Server-Sent Events (SSE) Module
//!
//! The SSE module provides support for Server-Sent Events, enabling real-time server-to-client
//! streaming of data over HTTP.
//!
//! ## Overview
//!
//! Server-Sent Events allow servers to push updates to clients over a long-lived HTTP connection.
//! This is useful for:
//! - Real-time notifications
//! - Live dashboards
//! - Progress updates for long-running operations
//! - Event streams and logs
//!
//! ## Architecture
//!
//! The SSE implementation uses channels for communication:
//!
//! - **[`SseSender`]** - Producer side that sends events
//! - **[`SseReceiver`]** - Consumer side that formats events for streaming
//! - **[`channel()`]** - Creates a new SSE channel pair
//!
//! ## Usage
//!
//! ```rust
//! use brrtrouter::sse;
//!
//! // Create a channel
//! let (sender, receiver) = sse::channel();
//!
//! // Send events
//! sender.send("Event 1");
//! sender.send("Event 2");
//! sender.send("Event 3");
//!
//! // Collect events as SSE-formatted string
//! let response = receiver.collect();
//! ```
//!
//! ## SSE Format
//!
//! Events are formatted according to the SSE specification:
//!
//! ```text
//! data: Event 1
//!
//! data: Event 2
//!
//! data: Event 3
//!
//! ```
//!
//! ## Handler Example
//!
//! ```rust,ignore
//! use brrtrouter::sse;
//! use brrtrouter::dispatcher::HandlerResponse;
//!
//! fn stream_events(_req: HandlerRequest) -> HandlerResponse {
//!     let (sender, receiver) = sse::channel();
//!     
//!     // Spawn coroutine to send events
//!     may::go!(move || {
//!         for i in 0..10 {
//!             sender.send(format!("Event {}", i));
//!             std::thread::sleep(std::time::Duration::from_secs(1));
//!         }
//!     });
//!     
//!     // Return SSE response
//!     HandlerResponse::new(200)
//!         .header("Content-Type", "text/event-stream")
//!         .header("Cache-Control", "no-cache")
//!         .body(receiver.collect())
//! }
//! ```
//!
//! ## Client-Side
//!
//! Clients consume SSE streams using the JavaScript EventSource API:
//!
//! ```javascript
//! const events = new EventSource('/stream_events');
//! events.onmessage = (event) => {
//!     console.log('Received:', event.data);
//! };
//! ```
//!
//! ## Performance
//!
//! - Uses `may` coroutine channels for efficient communication
//! - Lock-free message passing
//! - Minimal per-event overhead
//! - Suitable for thousands of concurrent streams

use may::sync::mpsc;

/// Sender side of an SSE channel.
///
/// Clone this to send events from multiple coroutines.
#[derive(Clone)]
pub struct SseSender {
    tx: mpsc::Sender<String>,
}

impl SseSender {
    pub fn send(&self, data: impl Into<String>) {
        let _ = self.tx.send(data.into());
    }
}

/// Receiver side that converts queued events into `text/event-stream` frames.
pub struct SseReceiver {
    rx: mpsc::Receiver<String>,
}

impl SseReceiver {
    /// Collect all events from the channel and return a single string containing
    /// properly formatted SSE frames.
    pub fn collect(self) -> String {
        let mut out = String::new();
        let rx = self.rx;
        while let Ok(msg) = rx.recv() {
            out.push_str("data: ");
            out.push_str(&msg);
            out.push_str("\n\n");
        }
        out
    }
}

/// Create a new SSE channel returning the sender and receiver halves.
pub fn channel() -> (SseSender, SseReceiver) {
    let (tx, rx) = mpsc::channel();
    (SseSender { tx }, SseReceiver { rx })
}
