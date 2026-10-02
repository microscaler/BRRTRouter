//! The runtime watchdog (2 Oct 2026 orders wedge): a `may` worker blocked in an OS wait must turn
//! the verdict - and so `/health` - unhealthy. Its own test binary: it sets the worker count and
//! deliberately blocks a worker, which must not touch other tests' runtimes.

use brrtrouter::runtime_watchdog::{Watchdog, WatchdogConfig};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn run_for(w: &Watchdog, every: Duration, d: Duration) {
    let end = Instant::now() + d;
    while Instant::now() < end {
        w.tick();
        std::thread::sleep(every);
    }
}

#[test]
fn a_worker_blocked_in_an_os_wait_turns_health_unhealthy() {
    may::config().set_workers(2);
    let every = Duration::from_millis(20);
    let w = Watchdog::new(WatchdogConfig {
        interval: every,
        stall_after: Duration::from_millis(300),
        window: Duration::from_secs(2),
        unhealthy_fraction: 0.25,
        min_samples: 10,
    });
    run_for(&w, every, Duration::from_millis(800));
    assert!(
        w.verdict().healthy,
        "healthy runtime judged starved: {:?}",
        w.verdict()
    );

    // What a handler waiting on a std/crossbeam channel does: blocks the may worker under it.
    let (_keep_sender, rx) = std::sync::mpsc::channel::<()>();
    let rx = Arc::new(Mutex::new(rx));
    let _blocked = unsafe {
        may::coroutine::spawn(move || {
            let _ = rx.lock().unwrap().recv();
        })
    };
    run_for(&w, every, Duration::from_millis(2500));
    let v = w.verdict();
    assert!(!v.healthy, "a blocked worker went unnoticed: {v:?}");
    assert_eq!(v.stalled_workers_estimate(), 1, "{v:?}");
}
