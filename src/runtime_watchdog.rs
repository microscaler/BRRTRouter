//! Detects `may` worker starvation and makes `/health` say so.
//!
//! Handlers run as coroutines on `may`'s worker threads, and each worker thread is also the epoll
//! selector for the sockets hashed to it. A handler that makes an OS-blocking call - waiting on a
//! crossbeam or `std::sync::mpsc` channel, holding a `std::sync::Mutex` across a park, a blocking
//! socket - does not park its coroutine: it takes the whole worker thread, and with it every
//! coroutine queued there and every socket on that selector. Nothing else notices. On 2 Oct 2026
//! the PriceWhisperer orders pod lost most of its 49 workers to calls into a hung IB Gateway; every
//! Postgres reply then missed its budget, and `/health` - answered by the workers that were left -
//! stayed green for seven hours, so Kubernetes never restarted it.
//!
//! The watchdog is an OS thread that spawns one probe coroutine per tick. A coroutine spawned from
//! outside the runtime goes to one worker's global queue (round-robin), and only that worker ever
//! collects it: a probe that has not run after [`WatchdogConfig::stall_after`] means its worker is
//! not running coroutines at all. The share of overdue probes over the last minute estimates the
//! share of stalled workers. Once it reaches [`WatchdogConfig::unhealthy_fraction`], `/health`
//! answers 503 (so the liveness probe restarts the pod) and the transition is printed to stderr
//! with a summary of what the process's threads are blocked in.
//!
//! It starts itself on the first `/health` request, so every service gets it without code changes.
//! Settings: `BRRTR_WATCHDOG=off` disables it; `BRRTR_WATCHDOG_STALL_SECS` (default 10) and
//! `BRRTR_WATCHDOG_UNHEALTHY_FRACTION` (default 0.25) tune it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Tuning for [`Watchdog`].
#[derive(Clone, Debug)]
pub struct WatchdogConfig {
    /// How often a probe coroutine is spawned.
    pub interval: Duration,
    /// A probe that has not run after this long counts as stalled.
    pub stall_after: Duration,
    /// How far back (beyond `stall_after`) probes are judged.
    pub window: Duration,
    /// Share of judged probes that must be stalled for the runtime to be called starved.
    pub unhealthy_fraction: f64,
    /// Fewer judged probes than this give no verdict (the first seconds after start).
    pub min_samples: usize,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(1),
            stall_after: Duration::from_secs(10),
            window: Duration::from_secs(60),
            unhealthy_fraction: 0.25,
            min_samples: 10,
        }
    }
}

impl WatchdogConfig {
    /// Defaults with the `BRRTR_WATCHDOG_*` overrides applied; `None` when `BRRTR_WATCHDOG=off`.
    pub fn from_env() -> Option<Self> {
        if std::env::var("BRRTR_WATCHDOG")
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "off" | "false" | "0"
                )
            })
            .unwrap_or(false)
        {
            return None;
        }
        let mut c = Self::default();
        if let Some(s) = std::env::var("BRRTR_WATCHDOG_STALL_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
        {
            c.stall_after = Duration::from_secs(s.max(1));
        }
        if let Some(f) = std::env::var("BRRTR_WATCHDOG_UNHEALTHY_FRACTION")
            .ok()
            .and_then(|v| v.trim().parse::<f64>().ok())
        {
            c.unhealthy_fraction = f.clamp(0.01, 1.0);
        }
        Some(c)
    }
}

/// The watchdog's current judgement of the runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    /// False once the stalled share reaches the threshold.
    pub healthy: bool,
    /// Probes judged (older than `stall_after`, inside the window).
    pub judged: usize,
    /// Of those, how many never ran.
    pub stalled: usize,
    /// `may` worker threads in the runtime.
    pub workers: usize,
}

impl Verdict {
    /// Estimated stalled workers (`stalled / judged` of the pool), rounded.
    pub fn stalled_workers_estimate(&self) -> usize {
        if self.judged == 0 {
            return 0;
        }
        ((self.stalled as f64 / self.judged as f64) * self.workers as f64).round() as usize
    }
}

struct Probe {
    sent: Instant,
    ran: Arc<AtomicBool>,
}

/// One watchdog over the process's `may` runtime. See the module docs.
pub struct Watchdog {
    cfg: WatchdogConfig,
    probes: Mutex<VecDeque<Probe>>,
    verdict: Mutex<Verdict>,
    workers: usize,
}

impl Watchdog {
    /// A watchdog that is not yet probing; drive it with [`Watchdog::tick`] or [`Watchdog::spawn`].
    pub fn new(cfg: WatchdogConfig) -> Arc<Self> {
        let workers = may::config().get_workers();
        Arc::new(Self {
            cfg,
            probes: Mutex::new(VecDeque::new()),
            verdict: Mutex::new(Verdict {
                healthy: true,
                judged: 0,
                stalled: 0,
                workers,
            }),
            workers,
        })
    }

    /// Start the probing thread.
    pub fn spawn(self: &Arc<Self>) -> std::io::Result<()> {
        let me = Arc::clone(self);
        std::thread::Builder::new()
            .name("brrtr-watchdog".into())
            .spawn(move || loop {
                me.tick();
                std::thread::sleep(me.cfg.interval);
            })
            .map(|_| ())
    }

    /// Spawn one probe and re-judge. Called by the probing thread every `interval`.
    pub fn tick(&self) {
        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);
        // SAFETY: the closure owns everything it touches (an Arc'd flag) and never blocks.
        let _detached = unsafe {
            may::coroutine::Builder::new()
                .name("brrtr-watchdog-probe".to_owned())
                .stack_size(0x1000 * 4)
                .spawn(move || flag.store(true, Ordering::Release))
        };
        let now = Instant::now();
        let mut probes = self.probes.lock().unwrap_or_else(|e| e.into_inner());
        probes.push_back(Probe { sent: now, ran });
        let horizon = self.cfg.stall_after + self.cfg.window;
        while probes
            .front()
            .is_some_and(|p| now.duration_since(p.sent) > horizon)
        {
            probes.pop_front();
        }
        let (mut judged, mut stalled) = (0usize, 0usize);
        for p in probes.iter() {
            if now.duration_since(p.sent) >= self.cfg.stall_after {
                judged += 1;
                if !p.ran.load(Ordering::Acquire) {
                    stalled += 1;
                }
            }
        }
        drop(probes);
        let starved = judged >= self.cfg.min_samples
            && (stalled as f64) >= self.cfg.unhealthy_fraction * judged as f64
            && stalled > 0;
        let next = Verdict {
            healthy: !starved,
            judged,
            stalled,
            workers: self.workers,
        };
        let mut cur = self.verdict.lock().unwrap_or_else(|e| e.into_inner());
        if cur.healthy != next.healthy {
            if next.healthy {
                eprintln!(
                    "[brrtrouter] may runtime recovered: {}/{} recent probes overdue",
                    next.stalled, next.judged
                );
            } else {
                eprintln!(
                    "[brrtrouter] MAY RUNTIME STARVED: {}/{} probe coroutines did not run within {:?} - about {} of {} may workers are blocked in OS calls (a handler or library is waiting on a non-may channel, lock or socket). /health now answers 503. Threads: {}",
                    next.stalled,
                    next.judged,
                    self.cfg.stall_after,
                    next.stalled_workers_estimate(),
                    next.workers,
                    thread_states()
                );
            }
        }
        *cur = next;
    }

    /// The latest judgement.
    pub fn verdict(&self) -> Verdict {
        self.verdict
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

static GLOBAL: OnceLock<Option<Arc<Watchdog>>> = OnceLock::new();

/// The process watchdog, started on first use (unless `BRRTR_WATCHDOG=off`). Call it once the
/// `may` runtime is configured; `/health` does.
pub fn global() -> Option<&'static Arc<Watchdog>> {
    GLOBAL
        .get_or_init(|| {
            let cfg = WatchdogConfig::from_env()?;
            let w = Watchdog::new(cfg);
            match w.spawn() {
                Ok(()) => Some(w),
                Err(e) => {
                    eprintln!("[brrtrouter] runtime watchdog could not start: {e}");
                    None
                }
            }
        })
        .as_ref()
}

/// What the process's threads are blocked in, from `/proc/self/task/*/syscall` (Linux only):
/// `futex` dominating means threads waiting on locks or non-may channels.
fn thread_states() -> String {
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        return "unavailable".into();
    };
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for t in tasks.flatten() {
        let p = t.path();
        let comm = std::fs::read_to_string(p.join("comm")).unwrap_or_default();
        let sys = std::fs::read_to_string(p.join("syscall")).unwrap_or_default();
        let what = match sys.split_whitespace().next() {
            Some("running") => "running".to_string(),
            Some(n) => syscall_name(n).to_string(),
            None => "?".to_string(),
        };
        *counts
            .entry(format!("{}:{}", comm.trim(), what))
            .or_default() += 1;
    }
    counts
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn syscall_name(nr: &str) -> &'static str {
    // x86_64 / aarch64 numbers for the waits that matter here.
    match (std::env::consts::ARCH, nr) {
        ("x86_64", "202") | ("aarch64", "98") => "futex",
        ("x86_64", "232") | ("x86_64", "281") | ("aarch64", "22") => "epoll_wait",
        ("x86_64", "0") | ("aarch64", "63") => "read",
        ("x86_64", "7") | ("x86_64", "271") | ("aarch64", "73") => "poll",
        ("x86_64", "23") | ("x86_64", "270") | ("aarch64", "72") => "select",
        ("x86_64", "35") | ("x86_64", "230") | ("aarch64", "101") | ("aarch64", "115") => "sleep",
        ("x86_64", "45") | ("x86_64", "47") | ("aarch64", "207") | ("aarch64", "212") => "recv",
        ("x86_64", "42") | ("aarch64", "203") => "connect",
        _ => "other",
    }
}
