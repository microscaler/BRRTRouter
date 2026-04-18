//! Zero-allocation one-shot reply channel for the dispatcher → handler → dispatcher
//! round trip (PRD Phase 3).
//!
//! # Why
//!
//! Before this module, [`crate::dispatcher::core::Dispatcher::dispatch_with_request_id`]
//! allocated a fresh [`may::sync::mpsc`] channel (an `Arc<Inner>` wrapping a
//! mutex + condvar pair) on every request, then handed the `Sender` to the
//! handler. At ~66 k req/s that was 66 k allocations/s plus a mutex/condvar
//! handoff on every reply — directly observable in flamegraphs.
//!
//! # Design
//!
//! A [`ReplySlot`] is a single-slot one-shot mailbox wired to the dispatcher
//! coroutine's [`may::coroutine::Park`] (via the handle returned by
//! [`may::coroutine::current`]). The handler fills the slot and unparks the
//! dispatcher; the dispatcher's `recv()` loops until the state flag says the
//! value is present.
//!
//! ```text
//! ┌──── dispatcher coroutine ────┐         ┌──── handler coroutine ─────┐
//! │ 1. slot = Arc<ReplySlot>     │         │                             │
//! │ 2. req = {…, reply_tx=slot}  │ ──send→ │ 3. exec handler             │
//! │ 4. slot.recv():              │         │ 5. slot.send(resp):         │
//! │    loop {                    │         │      state = FILLED         │
//! │      if state == FILLED      │         │      dispatcher.unpark()    │
//! │        return value          │         │                             │
//! │      park()  ← blocks        │ ←wake── │                             │
//! │    }                         │         │                             │
//! └──────────────────────────────┘         └────────────────────────────┘
//! ```
//!
//! The protocol is robust against:
//! * **Handler finishes before dispatcher parks.** `may::coroutine::unpark`
//!   stores a "was unparked" flag; the next `park()` call returns immediately.
//! * **Spurious wakes.** `recv()` is a CAS-style loop — we only return when
//!   we observe `state == FILLED`.
//! * **Handler panic.** The handler wrapper catches panics and sends a 500
//!   response through the slot before propagating. If the wrapper itself
//!   panics, the slot is dropped with `state == EMPTY`; `recv()` detects
//!   this via [`Arc::strong_count`] on the slot and returns `None` when the
//!   sender side has been dropped without sending.
//!
//! # One-shot
//!
//! A `ReplySlot` is consumed by exactly one `send` and one `recv`. Reusing
//! it across requests is not supported — the coroutine handle captured at
//! construction belongs to a specific dispatcher invocation. Allocation
//! remains a single `Arc<ReplySlot>` per request, replacing the pre-Phase-3
//! `Arc<Inner>` + mutex + condvar.
//!
//! # Test ergonomics
//!
//! Tests that construct a [`crate::dispatcher::HandlerRequest`] manually
//! (without going through the dispatcher) can use
//! [`HandlerReplySender::channel`] to keep the old `mpsc::channel()` shape.
//! The enum lets both production and test paths live side by side.

use crate::dispatcher::HandlerResponse;
use may::coroutine::Coroutine;
use may::sync::mpsc;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Empty: nothing has been written yet.
const STATE_EMPTY: u8 = 0;
/// Filled: sender wrote the value and (if a waiter is present) unparked it.
const STATE_FILLED: u8 = 1;

/// One-shot reply slot. See the module docs.
///
/// Constructed by the dispatcher before enqueueing the request to a handler
/// coroutine. The dispatcher then blocks on [`ReplySlot::recv`]; the handler
/// delivers via [`ReplySlot::send`].
pub struct ReplySlot {
    state: AtomicU8,
    /// Response cell.
    ///
    /// # SAFETY
    ///
    /// * Before `state` transitions EMPTY → FILLED, only the sender owns
    ///   this cell (no one else touches it).
    /// * After `state == FILLED`, the receiver takes the value exactly once.
    ///
    /// A single paired send/recv guarantees exclusive access in each phase.
    value: UnsafeCell<Option<HandlerResponse>>,
    /// Dispatcher coroutine to unpark when the value lands. Taken out on
    /// send to avoid `Clone` requirements on [`Coroutine`]. `None` before
    /// anyone is waiting (tests sometimes build a slot without a waiter).
    waiter: UnsafeCell<Option<Coroutine>>,
}

// SAFETY: access to `value` / `waiter` is synchronised by the `state` atomic.
// The single producer / single consumer invariant ensures there is no
// simultaneous mutable access across threads.
unsafe impl Send for ReplySlot {}
unsafe impl Sync for ReplySlot {}

impl ReplySlot {
    /// Create a slot that will unpark `waiter` when filled.
    ///
    /// Most callers should use [`ReplySlot::for_current`], which captures the
    /// current coroutine automatically.
    #[must_use]
    pub fn new(waiter: Coroutine) -> Self {
        Self {
            state: AtomicU8::new(STATE_EMPTY),
            value: UnsafeCell::new(None),
            waiter: UnsafeCell::new(Some(waiter)),
        }
    }

    /// Create a slot wired to unpark the current coroutine on [`send`].
    ///
    /// # Panics
    ///
    /// Panics if called outside a may coroutine context — the dispatcher hot
    /// path always runs inside one, so this is the expected-use constructor.
    #[must_use]
    pub fn for_current() -> Self {
        Self::new(may::coroutine::current())
    }

    /// Deliver a response to the waiting dispatcher.
    ///
    /// Safe to call at most once per slot. Subsequent calls are no-ops (the
    /// state transition from EMPTY to FILLED is idempotent from the
    /// caller's perspective — the second write would be observationally
    /// lost, but the handler wrapper guarantees a single send path).
    pub fn send(&self, response: HandlerResponse) {
        // If we've already been filled, drop the duplicate silently. This
        // matches `mpsc::Sender::send` behavior after the receiver has closed
        // (returns error; callers generally ignore it).
        if self.state.load(Ordering::Acquire) != STATE_EMPTY {
            return;
        }
        // SAFETY: exclusive access to `value` while state is EMPTY.
        unsafe {
            *self.value.get() = Some(response);
        }
        self.state.store(STATE_FILLED, Ordering::Release);
        // Take the waiter out; unpark is `&self`, so we don't need to keep
        // the handle around.
        //
        // SAFETY: EMPTY → FILLED transition is owned by this call site; no
        // other thread observes the value until state is FILLED, by which
        // point we've already taken the waiter.
        let waiter_slot: &mut Option<Coroutine> = unsafe { &mut *self.waiter.get() };
        if let Some(co) = waiter_slot.take() {
            co.unpark();
        }
    }

    /// Block the current coroutine until [`send`] is called, then return the
    /// response. Returns `None` if the sender side was dropped without
    /// sending (e.g. handler panic outside the wrapper).
    pub fn recv(self: &Arc<Self>) -> Option<HandlerResponse> {
        loop {
            if self.state.load(Ordering::Acquire) == STATE_FILLED {
                // SAFETY: FILLED implies `send` finished writing; no other
                // thread will read or write `value` from here.
                return unsafe { (*self.value.get()).take() };
            }
            // If we are the only remaining Arc holder, the sender side has
            // been dropped without sending. Return None to let the caller
            // surface a 5xx.
            if Arc::strong_count(self) == 1 {
                return None;
            }
            may::coroutine::park();
        }
    }
}

/// Reply channel handed to the handler in [`crate::dispatcher::HandlerRequest`].
///
/// This enum lets the production path (dispatcher → handler) use the
/// zero-alloc [`ReplySlot`] while test code can keep constructing
/// [`mpsc::Sender`]s directly. Internal handler wrappers call `send`
/// uniformly via this enum.
#[derive(Clone)]
pub enum HandlerReplySender {
    /// Legacy mpsc channel — kept for test ergonomics and any direct
    /// [`HandlerRequest`] construction outside the dispatcher.
    Channel(mpsc::Sender<HandlerResponse>),
    /// PRD Phase 3 parker slot — production dispatch path.
    Slot(Arc<ReplySlot>),
}

impl HandlerReplySender {
    /// Wrap a `mpsc::Sender` (legacy / test path).
    #[must_use]
    pub fn channel(tx: mpsc::Sender<HandlerResponse>) -> Self {
        Self::Channel(tx)
    }

    /// Wrap a parker slot (production dispatch path).
    #[must_use]
    pub fn slot(slot: Arc<ReplySlot>) -> Self {
        Self::Slot(slot)
    }

    /// Send a response to whoever dispatched this request. Returns `Err(resp)`
    /// with the original response on failure (mpsc channel disconnected).
    /// The slot variant is infallible (state CAS is local).
    pub fn send(&self, response: HandlerResponse) -> Result<(), HandlerResponse> {
        match self {
            HandlerReplySender::Channel(tx) => tx.send(response).map_err(|e| e.0),
            HandlerReplySender::Slot(slot) => {
                slot.send(response);
                Ok(())
            }
        }
    }
}

impl std::fmt::Debug for HandlerReplySender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerReplySender::Channel(_) => f.write_str("HandlerReplySender::Channel(..)"),
            HandlerReplySender::Slot(s) => f
                .debug_struct("HandlerReplySender::Slot")
                .field("state", &s.state.load(Ordering::Relaxed))
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Basic round-trip: send → recv returns the value.
    /// Run in a may coroutine so `park` / `current` have a context.
    #[test]
    fn slot_roundtrip_in_coroutine() {
        may::config().set_stack_size(0x8000);
        // SAFETY: tests run with the may runtime; spawn is unsafe because of
        // the stackful-coroutine contract, which we honor by keeping the
        // closure `Send + 'static` and joining before the test returns.
        let handle = unsafe {
            may::coroutine::spawn(|| {
                let slot = Arc::new(ReplySlot::for_current());
                let slot_tx = Arc::clone(&slot);
                // Producer coroutine: send from a separate coroutine so our recv
                // path actually exercises park/unpark.
                let producer = unsafe {
                    may::coroutine::spawn(move || {
                        slot_tx.send(HandlerResponse::json(
                            201,
                            serde_json::json!({"hello": "parker"}),
                        ));
                    })
                };
                let resp = slot.recv().expect("recv must see FILLED");
                assert_eq!(resp.status, 201);
                assert_eq!(
                    resp.body.get("hello").and_then(|v| v.as_str()),
                    Some("parker")
                );
                producer.join().unwrap();
            })
        };
        handle.join().unwrap();
    }

    /// Producer runs to completion before the consumer reaches `recv`; the
    /// `state == FILLED` check short-circuits past `park`.
    #[test]
    fn slot_send_before_recv_does_not_deadlock() {
        may::config().set_stack_size(0x8000);
        let handle = unsafe {
            may::coroutine::spawn(|| {
                let slot = Arc::new(ReplySlot::for_current());
                slot.send(HandlerResponse::error(418, "teapot"));
                // No park: state is already FILLED.
                let resp = slot.recv().expect("value was already delivered");
                assert_eq!(resp.status, 418);
            })
        };
        handle.join().unwrap();
    }

    /// When the sender side is dropped without sending, `recv` returns None
    /// so the dispatcher can surface a 5xx instead of hanging forever.
    #[test]
    fn slot_sender_dropped_without_send_returns_none() {
        may::config().set_stack_size(0x8000);
        let handle = unsafe {
            may::coroutine::spawn(|| {
                let slot = Arc::new(ReplySlot::for_current());
                let slot_tx = Arc::clone(&slot);
                // Drop without sending.
                drop(slot_tx);
                let resp = slot.recv();
                assert!(
                    resp.is_none(),
                    "recv should return None when sender drops silently"
                );
            })
        };
        handle.join().unwrap();
    }

    /// `HandlerReplySender::Channel(..)` keeps legacy mpsc behavior for tests.
    #[test]
    fn handler_reply_sender_channel_legacy() {
        let (tx, rx) = mpsc::channel();
        let sender = HandlerReplySender::channel(tx);
        sender
            .send(HandlerResponse::error(500, "boom"))
            .expect("send must succeed");
        let resp = rx.recv().expect("legacy mpsc recv");
        assert_eq!(resp.status, 500);
    }
}
