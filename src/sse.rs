use may::sync::mpsc;

/// Sender side of an SSE channel.
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
