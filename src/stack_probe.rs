//! Per-handler coroutine stack usage, measured from page residency.
//!
//! A coroutine stack is an anonymous `mmap` region. Linux backs a page with memory only once it is touched, and
//! the stack grows downward, so the lowest resident page of the mapping is the deepest the handler has reached.
//! [`StackProbe`] reads residency with `mincore(2)`: one syscall, nothing painted, no memory cost, accurate to a
//! page (4 KiB).
//!
//! Every `BRRTR_STACK_PROBE_EVERY`-th request of a handler (default 16, `0` turns the probe off):
//! 1. before the request: record the deepest use since the previous sample (the handler's peak), then
//!    `madvise(MADV_DONTNEED)` the pages below the current stack pointer, so the request starts from a clean stack;
//! 2. after it: record the request's own depth in the histogram.
//!
//! The `MADV_DONTNEED` also hands the pages of an occasional deep request back to the kernel instead of leaving
//! them resident for the life of the pod; the next deep request pays a few page faults.
//!
//! Exposed on `/metrics` as `brrtrouter_handler_stack_used_bytes` (histogram),
//! `brrtrouter_handler_stack_peak_bytes` and `brrtrouter_handler_stack_size_bytes` (gauges), each labelled
//! `handler`. Only the handler's own coroutine is measured: work it hands to other coroutines or threads (a
//! database pool's workers, say) is not counted against it. Off (a no-op) on non-Linux targets.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Histogram bucket upper bounds in bytes: 4 KiB .. 4 MiB (powers of two), then `+Inf`.
pub const BUCKETS: [usize; 11] = [
    4 << 10,
    8 << 10,
    16 << 10,
    32 << 10,
    64 << 10,
    128 << 10,
    256 << 10,
    512 << 10,
    1 << 20,
    2 << 20,
    4 << 20,
];

/// Stack statistics of one handler (all its coroutines: a worker pool's workers share one entry).
#[derive(Debug, Default)]
pub struct HandlerStackStats {
    /// Bytes actually mapped for the stack (what `may` allocated, guard page excluded).
    pub size: AtomicUsize,
    /// Deepest use observed since start.
    pub peak: AtomicUsize,
    buckets: [AtomicU64; BUCKETS.len()],
    count: AtomicU64,
    sum: AtomicU64,
}

impl HandlerStackStats {
    /// Record one request's stack depth.
    pub fn observe(&self, used: usize) {
        self.peak.fetch_max(used, Ordering::Relaxed);
        for (i, le) in BUCKETS.iter().enumerate() {
            if used <= *le {
                if let Some(b) = self.buckets.get(i) {
                    b.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(used as u64, Ordering::Relaxed);
    }

    /// Requests observed so far.
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

type Registry = Mutex<BTreeMap<String, Arc<HandlerStackStats>>>;

fn registry() -> &'static Registry {
    static REG: OnceLock<Registry> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// The statistics entry of `handler`, created on first use.
pub fn stats_for(handler: &str) -> Arc<HandlerStackStats> {
    let mut reg = registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    reg.entry(handler.to_owned()).or_default().clone()
}

/// The deepest stack use any handler has reached (bytes); 0 before the first sample.
pub fn max_peak() -> usize {
    let reg = registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    reg.values()
        .map(|s| s.peak.load(Ordering::Relaxed))
        .max()
        .unwrap_or(0)
}

fn escape_label(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Append the per-handler stack metrics in Prometheus text format.
///
/// Only handlers with at least one sampled request are rendered: every registered route has a handler coroutine,
/// most never see traffic outside production, and each rendered handler is 15 series in a scraped (and billed)
/// Prometheus.
pub fn render_prometheus(out: &mut String) {
    let all = registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let reg: BTreeMap<&String, &Arc<HandlerStackStats>> =
        all.iter().filter(|(_, s)| s.count() > 0).collect();
    if reg.is_empty() {
        return;
    }
    out.push_str("# HELP brrtrouter_handler_stack_size_bytes Coroutine stack mapped for the handler (bytes)\n");
    out.push_str("# TYPE brrtrouter_handler_stack_size_bytes gauge\n");
    for (name, s) in reg.iter() {
        let _ = writeln!(
            out,
            "brrtrouter_handler_stack_size_bytes{{handler=\"{}\"}} {}",
            escape_label(name),
            s.size.load(Ordering::Relaxed)
        );
    }
    out.push_str("# HELP brrtrouter_handler_stack_peak_bytes Deepest stack use of the handler since start (bytes, page granularity)\n");
    out.push_str("# TYPE brrtrouter_handler_stack_peak_bytes gauge\n");
    for (name, s) in reg.iter() {
        let _ = writeln!(
            out,
            "brrtrouter_handler_stack_peak_bytes{{handler=\"{}\"}} {}",
            escape_label(name),
            s.peak.load(Ordering::Relaxed)
        );
    }
    out.push_str("# HELP brrtrouter_handler_stack_used_bytes Stack depth of sampled requests (bytes, page granularity)\n");
    out.push_str("# TYPE brrtrouter_handler_stack_used_bytes histogram\n");
    for (name, s) in reg.iter() {
        let h = escape_label(name);
        for (i, le) in BUCKETS.iter().enumerate() {
            let c = s.buckets.get(i).map_or(0, |b| b.load(Ordering::Relaxed));
            let _ = writeln!(
                out,
                "brrtrouter_handler_stack_used_bytes_bucket{{handler=\"{h}\",le=\"{le}\"}} {c}"
            );
        }
        let count = s.count.load(Ordering::Relaxed);
        let _ = writeln!(
            out,
            "brrtrouter_handler_stack_used_bytes_bucket{{handler=\"{h}\",le=\"+Inf\"}} {count}"
        );
        let _ = writeln!(
            out,
            "brrtrouter_handler_stack_used_bytes_sum{{handler=\"{h}\"}} {}",
            s.sum.load(Ordering::Relaxed)
        );
        let _ = writeln!(
            out,
            "brrtrouter_handler_stack_used_bytes_count{{handler=\"{h}\"}} {count}"
        );
    }
}

/// `BRRTR_STACK_PROBE_EVERY` (default 16; 0 = off).
fn sample_every() -> u32 {
    static EVERY: OnceLock<u32> = OnceLock::new();
    *EVERY.get_or_init(|| {
        std::env::var("BRRTR_STACK_PROBE_EVERY")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(16)
    })
}

/// A handler coroutine's stack probe. Create it at the top of the coroutine (it locates the stack from the current
/// stack pointer), then wrap each request in [`StackProbe::request`].
#[derive(Debug)]
pub struct StackProbe {
    stats: Arc<HandlerStackStats>,
    every: u32,
    n: u32,
    #[cfg(target_os = "linux")]
    stack: linux::Stack,
}

impl StackProbe {
    /// The probe of the calling coroutine, recorded under `handler`. `None` when the probe is off
    /// (`BRRTR_STACK_PROBE_EVERY=0`), on non-Linux targets, or when the stack mapping cannot be found.
    pub fn for_current(handler: &str) -> Option<Self> {
        let every = sample_every();
        if every == 0 {
            return None;
        }
        #[cfg(target_os = "linux")]
        {
            let stack = linux::Stack::current()?;
            let stats = stats_for(handler);
            stats.size.fetch_max(stack.len(), Ordering::Relaxed);
            Some(StackProbe {
                stats,
                every,
                n: 0,
                stack,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = handler;
            None
        }
    }

    /// Wrap one request: the returned guard records the request's depth when dropped (sampled requests only).
    #[inline]
    pub fn request(&mut self) -> ProbeGuard<'_> {
        self.n = self.n.wrapping_add(1);
        let sampled = self.n.is_multiple_of(self.every);
        if sampled {
            self.begin_sample();
        }
        ProbeGuard {
            probe: self,
            sampled,
        }
    }

    #[cfg(target_os = "linux")]
    fn begin_sample(&mut self) {
        // The deepest use since the previous sample, unsampled requests included, then a clean slate.
        if let Some(used) = self.stack.used() {
            self.stats.peak.fetch_max(used, Ordering::Relaxed);
        }
        self.stack.release_below_sp();
    }

    #[cfg(not(target_os = "linux"))]
    fn begin_sample(&mut self) {}

    #[cfg(target_os = "linux")]
    fn end_sample(&mut self) {
        if let Some(used) = self.stack.used() {
            self.stats.observe(used);
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn end_sample(&mut self) {}
}

/// Records the wrapped request's stack depth on drop (see [`StackProbe::request`]).
#[derive(Debug)]
pub struct ProbeGuard<'a> {
    probe: &'a mut StackProbe,
    sampled: bool,
}

impl Drop for ProbeGuard<'_> {
    fn drop(&mut self) {
        if self.sampled {
            self.probe.end_sample();
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::io::{BufRead, BufReader};

    /// The writable stack mapping of the calling coroutine (the guard page is a separate mapping).
    #[derive(Debug)]
    pub struct Stack {
        lo: usize,
        hi: usize,
        page: usize,
    }

    #[inline(never)]
    fn stack_pointer() -> usize {
        let marker = 0u8;
        std::ptr::addr_of!(marker) as usize
    }

    fn page_size() -> usize {
        // SAFETY: sysconf has no preconditions.
        let p = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        usize::try_from(p).ok().filter(|p| *p > 0).unwrap_or(4096)
    }

    impl Stack {
        /// Find the mapping that holds the current stack pointer in `/proc/self/maps` (once per coroutine).
        pub fn current() -> Option<Self> {
            let sp = stack_pointer();
            let f = std::fs::File::open("/proc/self/maps").ok()?;
            for line in BufReader::new(f).lines() {
                let line = line.ok()?;
                let range = line.split_whitespace().next()?;
                let (a, b) = range.split_once('-')?;
                let lo = usize::from_str_radix(a, 16).ok()?;
                let hi = usize::from_str_radix(b, 16).ok()?;
                if (lo..hi).contains(&sp) {
                    return Some(Stack {
                        lo,
                        hi,
                        page: page_size(),
                    });
                }
            }
            None
        }

        pub fn len(&self) -> usize {
            self.hi - self.lo
        }

        /// The lowest page the runtime keeps resident itself: `generator` writes its overflow canary at the bottom
        /// of the stack, so the first page is always resident and is skipped.
        fn floor(&self) -> usize {
            self.lo + self.page
        }

        /// Bytes from the top of the mapping down to its lowest resident page.
        pub fn used(&self) -> Option<usize> {
            let start = self.floor();
            if start >= self.hi {
                return None;
            }
            let pages = (self.hi - start) / self.page;
            let mut vec = vec![0u8; pages];
            // SAFETY: [start, hi) lies inside our own mapping; `vec` holds one byte per page.
            let rc = unsafe {
                libc::mincore(
                    start as *mut libc::c_void,
                    self.hi - start,
                    vec.as_mut_ptr().cast(),
                )
            };
            if rc != 0 {
                return None;
            }
            let first = vec.iter().position(|b| b & 1 == 1)?;
            Some(self.hi - (start + first * self.page))
        }

        /// Drop the pages below the current stack pointer (one page of margin): they hold no live frames, so the
        /// kernel may reclaim them and the next touch faults in a zero page.
        pub fn release_below_sp(&self) {
            let sp = stack_pointer();
            let top = (sp & !(self.page - 1)).saturating_sub(self.page);
            let start = self.floor();
            if top <= start {
                return;
            }
            // SAFETY: [start, top) is inside our stack mapping and below every live frame of this coroutine.
            unsafe {
                libc::madvise(start as *mut libc::c_void, top - start, libc::MADV_DONTNEED);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_buckets_are_cumulative() {
        let s = HandlerStackStats::default();
        s.observe(5 << 10);
        s.observe(300 << 10);
        assert_eq!(s.count(), 2);
        assert_eq!(s.peak.load(Ordering::Relaxed), 300 << 10);
        let at = |le: usize| {
            let i = BUCKETS.iter().position(|b| *b == le).unwrap_or(0);
            s.buckets.get(i).map_or(0, |b| b.load(Ordering::Relaxed))
        };
        assert_eq!(at(4 << 10), 0);
        assert_eq!(at(8 << 10), 1);
        assert_eq!(at(256 << 10), 1);
        assert_eq!(at(512 << 10), 2);
    }

    #[test]
    fn unsampled_handlers_are_not_rendered() {
        let _ = stats_for("never_called_handler");
        let mut out = String::new();
        render_prometheus(&mut out);
        assert!(!out.contains("never_called_handler"));
    }

    #[test]
    fn renders_labelled_metrics() {
        stats_for("render_test_handler").observe(12 << 10);
        let mut out = String::new();
        render_prometheus(&mut out);
        assert!(out.contains("brrtrouter_handler_stack_used_bytes_bucket{handler=\"render_test_handler\",le=\"16384\"} 1"));
        assert!(out.contains(
            "brrtrouter_handler_stack_used_bytes_count{handler=\"render_test_handler\"} 1"
        ));
        assert!(out.contains(
            "brrtrouter_handler_stack_peak_bytes{handler=\"render_test_handler\"} 12288"
        ));
    }

    /// In a real `may` coroutine: the probe finds the stack mapping and sees a deep frame.
    #[cfg(target_os = "linux")]
    #[test]
    fn measures_a_deep_frame_in_a_coroutine() {
        #[inline(never)]
        #[allow(clippy::large_stack_arrays)] // the point: a deep frame
        fn deep(n: usize) -> u8 {
            let buf = std::hint::black_box([n as u8; 64 << 10]);
            buf[n % buf.len()]
        }
        let h = unsafe {
            may::coroutine::Builder::new()
                .stack_size(0x40000) // words: 2 MiB
                .spawn(|| {
                    let mut probe =
                        StackProbe::for_current("deep_frame_test").expect("stack mapping");
                    // sample every request regardless of the env default
                    probe.every = 1;
                    {
                        let _g = probe.request();
                        std::hint::black_box(deep(7));
                    }
                    {
                        let _g = probe.request();
                    }
                })
                .expect("spawn")
        };
        h.join().expect("join");
        let s = stats_for("deep_frame_test");
        assert_eq!(s.count(), 2);
        assert!(
            s.peak.load(Ordering::Relaxed) >= 64 << 10,
            "peak {}",
            s.peak.load(Ordering::Relaxed)
        );
        assert!(s.size.load(Ordering::Relaxed) >= 1 << 20);
        // the second, shallow request starts from a released stack and stays small
        assert!(
            s.buckets.get(3).map_or(0, |b| b.load(Ordering::Relaxed)) >= 1,
            "shallow request not under 32 KiB"
        );
    }
}
