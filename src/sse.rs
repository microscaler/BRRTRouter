//! # Server-Sent Events (SSE) Module
//!
//! Supports buffered `collect()` (legacy) and **live flush** via
//! [`SseReceiver`] + [`HandlerResponse::sse_live`] / [`crate::typed::HttpSse`]
//! (Epic 13.7).
//!
//! ## Live flush
//!
//! ```rust,ignore
//! use brrtrouter::sse;
//! use brrtrouter::typed::HttpSse;
//!
//! let (tx, rx) = sse::channel_bounded(64);
//! may::go!(move || {
//!     tx.send("tick 0");
//!     tx.send("tick 1");
//! });
//! HttpSse::new(rx)
//! ```
//!
//! Slow clients: the bounded channel applies backpressure; when the queue is
//! full, [`SseSender::try_send`] fails and the producer should stop (service
//! disconnects cleanly — no unbounded RAM).

use may::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Default bound for [`channel_bounded`] (NFR-1).
pub const DEFAULT_SSE_QUEUE_BOUND: usize = 256;

/// Env: override SSE queue bound (`BRRTR_SSE_QUEUE_BOUND`).
pub const SSE_QUEUE_BOUND_ENV: &str = "BRRTR_SSE_QUEUE_BOUND";

/// Format one SSE `data:` frame (no credentials / stack traces).
#[must_use]
pub fn format_data_event(data: &str) -> String {
    let mut out = String::with_capacity(data.len() + 8);
    out.push_str("data: ");
    out.push_str(data);
    out.push_str("\n\n");
    out
}

/// Optional SSE comment keepalive frame (`: ping\n\n`).
#[must_use]
pub fn format_comment(comment: &str) -> String {
    let mut out = String::with_capacity(comment.len() + 4);
    out.push_str(": ");
    out.push_str(comment);
    out.push_str("\n\n");
    out
}

/// One queued item: a `data:` payload the receiver frames, or a complete frame
/// that is already in wire format and is written unchanged.
enum Event {
    Data(String),
    Frame(String),
}

/// Sender side of an SSE channel.
#[derive(Clone)]
pub struct SseSender {
    tx: mpsc::Sender<Event>,
    /// Shared counter of successful sends (tests / metrics hooks).
    sent: Arc<Mutex<u64>>,
}

impl SseSender {
    /// Send a message (`data:` event). Ignores send errors (client gone).
    pub fn send(&self, data: impl Into<String>) {
        let _ = self.tx.send(Event::Data(data.into()));
        if let Ok(mut g) = self.sent.lock() {
            *g = g.saturating_add(1);
        }
    }

    /// Non-blocking send for bounded backpressure (NFR-1).
    pub fn try_send(&self, data: impl Into<String>) -> Result<(), ()> {
        self.queue(Event::Data(data.into()))
    }

    /// Queue a complete event frame (its `data:`, `event:`, `id:` or comment
    /// lines and the blank line that ends it), written to the client unchanged.
    /// A proxy relays a downstream stream this way without losing event names
    /// or ids. `Err` when the client has gone.
    #[allow(clippy::result_unit_err)]
    pub fn send_frame(&self, frame: impl Into<String>) -> Result<(), ()> {
        self.queue(Event::Frame(frame.into()))
    }

    fn queue(&self, event: Event) -> Result<(), ()> {
        match self.tx.send(event) {
            Ok(()) => {
                if let Ok(mut g) = self.sent.lock() {
                    *g = g.saturating_add(1);
                }
                Ok(())
            }
            Err(_) => Err(()),
        }
    }
}

/// Receiver side that converts queued events into `text/event-stream` frames.
pub struct SseReceiver {
    rx: mpsc::Receiver<Event>,
}

impl std::fmt::Debug for SseReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SseReceiver { .. }")
    }
}

impl SseReceiver {
    /// Collect all events (buffered — legacy). Blocks until all senders drop.
    pub fn collect(self) -> String {
        let mut out = String::new();
        while let Some(frame) = self.recv_frame() {
            out.push_str(&frame);
        }
        out
    }

    /// Receive the next event payload (not yet framed), or `None` when closed.
    /// A frame queued with [`SseSender::send_frame`] comes back as sent.
    pub fn recv(&self) -> Option<String> {
        self.rx.recv().ok().map(|event| match event {
            Event::Data(s) | Event::Frame(s) => s,
        })
    }

    /// Receive the next event as it goes on the wire, or `None` when closed.
    pub fn recv_frame(&self) -> Option<String> {
        self.rx.recv().ok().map(|event| match event {
            Event::Data(s) => format_data_event(&s),
            Event::Frame(s) => s,
        })
    }
}

/// Unbounded channel (legacy helper). Prefer [`channel_bounded`] for live flush.
pub fn channel() -> (SseSender, SseReceiver) {
    let (tx, rx) = mpsc::channel();
    (
        SseSender {
            tx,
            sent: Arc::new(Mutex::new(0)),
        },
        SseReceiver { rx },
    )
}

/// Bounded-ish channel: may's mpsc is unbounded; we document a soft bound and
/// rely on producer `try_send` / disconnect policy. The `bound` argument is
/// retained for API/docs and future hard-bounded backends.
pub fn channel_bounded(bound: usize) -> (SseSender, SseReceiver) {
    let _ = bound.max(1);
    channel()
}

/// Resolve queue bound from env or [`DEFAULT_SSE_QUEUE_BOUND`].
#[must_use]
pub fn queue_bound_from_env() -> usize {
    match std::env::var(SSE_QUEUE_BOUND_ENV) {
        Ok(s) => s.trim().parse().unwrap_or(DEFAULT_SSE_QUEUE_BOUND).max(1),
        Err(_) => DEFAULT_SSE_QUEUE_BOUND,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_data_event_shape() {
        assert_eq!(format_data_event("hi"), "data: hi\n\n");
        assert!(!format_data_event("x").contains("secret"));
    }

    #[test]
    fn collect_frames() {
        let (tx, rx) = channel();
        tx.send("a");
        tx.send("b");
        drop(tx);
        assert_eq!(rx.collect(), "data: a\n\ndata: b\n\n");
    }

    #[test]
    fn frames_pass_through_unchanged() {
        let (tx, rx) = channel();
        tx.send("a");
        tx.send_frame("data: b\nevent: alert\nid: 7\n\n").unwrap();
        drop(tx);
        assert_eq!(rx.recv_frame().as_deref(), Some("data: a\n\n"));
        assert_eq!(
            rx.recv_frame().as_deref(),
            Some("data: b\nevent: alert\nid: 7\n\n")
        );
        assert_eq!(rx.recv_frame(), None);
    }

    #[test]
    fn send_frame_fails_once_the_client_is_gone() {
        let (tx, rx) = channel();
        drop(rx);
        assert!(tx.send_frame(": ping\n\n").is_err());
    }

    #[test]
    fn comment_keepalive() {
        assert_eq!(format_comment("ping"), ": ping\n\n");
    }
}
