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
use std::sync::atomic::{AtomicUsize, Ordering};
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

impl Event {
    fn len(&self) -> usize {
        match self {
            Self::Data(s) | Self::Frame(s) => s.len(),
        }
    }
}

/// What is queued between a sender and its receiver, and the most that may be
/// ([`channel_limited`]); `usize::MAX` for no limit.
struct Backlog {
    frames: AtomicUsize,
    bytes: AtomicUsize,
    max_frames: usize,
    max_bytes: usize,
}

impl Backlog {
    fn unlimited() -> Arc<Self> {
        Self::limited(usize::MAX, usize::MAX)
    }

    fn limited(max_frames: usize, max_bytes: usize) -> Arc<Self> {
        Arc::new(Self {
            frames: AtomicUsize::new(0),
            bytes: AtomicUsize::new(0),
            max_frames,
            max_bytes,
        })
    }

    /// Reserve room for one event of `len` bytes; `false` when the queue is full.
    fn reserve(&self, len: usize, enforce: bool) -> bool {
        let frames = self.frames.fetch_add(1, Ordering::AcqRel);
        let bytes = self.bytes.fetch_add(len, Ordering::AcqRel);
        if enforce && (frames >= self.max_frames || bytes.saturating_add(len) > self.max_bytes) {
            self.release(len);
            return false;
        }
        true
    }

    fn release(&self, len: usize) {
        self.frames.fetch_sub(1, Ordering::AcqRel);
        self.bytes.fetch_sub(len, Ordering::AcqRel);
    }
}

/// Sender side of an SSE channel.
#[derive(Clone)]
pub struct SseSender {
    tx: mpsc::Sender<Event>,
    /// Shared counter of successful sends (tests / metrics hooks).
    sent: Arc<Mutex<u64>>,
    backlog: Arc<Backlog>,
}

impl SseSender {
    /// Send a message (`data:` event). Ignores send errors (client gone).
    pub fn send(&self, data: impl Into<String>) {
        let event = Event::Data(data.into());
        let len = event.len();
        self.backlog.reserve(len, false);
        if self.tx.send(event).is_err() {
            self.backlog.release(len);
        }
        if let Ok(mut g) = self.sent.lock() {
            *g = g.saturating_add(1);
        }
    }

    /// Non-blocking send for bounded backpressure (NFR-1). `Err` when the client
    /// has gone, or when a [`channel_limited`] queue is full.
    pub fn try_send(&self, data: impl Into<String>) -> Result<(), ()> {
        self.queue(Event::Data(data.into()))
    }

    /// Queue a complete event frame (its `data:`, `event:`, `id:` or comment
    /// lines and the blank line that ends it), written to the client unchanged.
    /// A proxy relays a downstream stream this way without losing event names
    /// or ids. `Err` when the client has gone, or when a [`channel_limited`]
    /// queue is full (the producer should then stop: the client reconnects).
    #[allow(clippy::result_unit_err)]
    pub fn send_frame(&self, frame: impl Into<String>) -> Result<(), ()> {
        self.queue(Event::Frame(frame.into()))
    }

    fn queue(&self, event: Event) -> Result<(), ()> {
        let len = event.len();
        if !self.backlog.reserve(len, true) {
            return Err(());
        }
        match self.tx.send(event) {
            Ok(()) => {
                if let Ok(mut g) = self.sent.lock() {
                    *g = g.saturating_add(1);
                }
                Ok(())
            }
            Err(_) => {
                self.backlog.release(len);
                Err(())
            }
        }
    }
}

/// Receiver side that converts queued events into `text/event-stream` frames.
pub struct SseReceiver {
    rx: mpsc::Receiver<Event>,
    backlog: Arc<Backlog>,
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
        self.next().map(|event| match event {
            Event::Data(s) | Event::Frame(s) => s,
        })
    }

    fn next(&self) -> Option<Event> {
        let event = self.rx.recv().ok()?;
        self.backlog.release(event.len());
        Some(event)
    }

    /// Receive the next event as it goes on the wire, or `None` when closed.
    pub fn recv_frame(&self) -> Option<String> {
        self.next().map(|event| match event {
            Event::Data(s) => format_data_event(&s),
            Event::Frame(s) => s,
        })
    }
}

/// Unbounded channel (legacy helper). Prefer [`channel_bounded`] for live flush.
pub fn channel() -> (SseSender, SseReceiver) {
    with_backlog(Backlog::unlimited())
}

/// A channel whose queue is held to `max_frames` events and `max_bytes` of
/// payload: [`SseSender::try_send`] and [`SseSender::send_frame`] fail once it
/// is full, so a client that reads slower than the producer writes is
/// disconnected (and reconnects) instead of growing the queue without limit.
/// [`SseSender::send`] is fire-and-forget and is not held to the limits.
#[must_use]
pub fn channel_limited(max_frames: usize, max_bytes: usize) -> (SseSender, SseReceiver) {
    with_backlog(Backlog::limited(max_frames.max(1), max_bytes.max(1)))
}

fn with_backlog(backlog: Arc<Backlog>) -> (SseSender, SseReceiver) {
    let (tx, rx) = mpsc::channel();
    (
        SseSender {
            tx,
            sent: Arc::new(Mutex::new(0)),
            backlog: Arc::clone(&backlog),
        },
        SseReceiver { rx, backlog },
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
    fn a_limited_queue_refuses_frames_past_its_limits() {
        let (tx, rx) = channel_limited(2, 1024);
        tx.send_frame("data: a\n\n").unwrap();
        tx.send_frame("data: b\n\n").unwrap();
        assert!(
            tx.send_frame("data: c\n\n").is_err(),
            "third frame over the frame limit"
        );
        assert_eq!(rx.recv_frame().as_deref(), Some("data: a\n\n"));
        tx.send_frame("data: c\n\n").unwrap();

        let (tx, rx) = channel_limited(100, 16);
        tx.send_frame("data: 0123456\n\n").unwrap();
        assert!(tx.send_frame("data: 7\n\n").is_err(), "over the byte limit");
        assert!(rx.recv_frame().is_some());
        tx.send_frame("data: 7\n\n").unwrap();
    }

    #[test]
    fn an_unlimited_queue_takes_everything() {
        let (tx, rx) = channel();
        for i in 0..1000 {
            tx.send_frame(format!("data: {i}\n\n")).unwrap();
        }
        drop(tx);
        assert_eq!(rx.collect().matches("data: ").count(), 1000);
    }

    #[test]
    fn comment_keepalive() {
        assert_eq!(format_comment("ping"), ": ping\n\n");
    }
}
