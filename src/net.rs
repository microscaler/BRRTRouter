//! Name resolution and TCP connects that never block a `may` worker thread.
//!
//! Handlers run as coroutines on `may`'s worker threads. A call that blocks in the kernel - a
//! `std::net::TcpStream` read, `std::sync::Condvar::wait`, or the system resolver
//! (`getaddrinfo`, behind `ToSocketAddrs`) - does not park the coroutine: it parks the worker
//! thread, and every coroutine queued on that worker waits behind it (see
//! [`crate::runtime_watchdog`]). `may::net` sockets are coroutine-aware; the system resolver has
//! no coroutine-aware equivalent, so lookups run here on a few dedicated OS threads and the caller
//! waits on a `may` channel, which parks only the calling coroutine.
//!
//! # Caching
//!
//! Answers are cached for `BRRTR_DNS_TTL_SECS` (default 30). An expired answer is still served
//! while one background refresh runs, so a hot path pays for a lookup at most once per name; if
//! that refresh fails, the last good answer is kept (still expired, so the next lookup tries
//! again). Concurrent lookups of one name share a single resolution. Failures are cached for
//! `BRRTR_DNS_NEGATIVE_TTL_SECS` (default 2). [`resolve_uncached`] skips the cache for callers
//! that just failed to connect to a cached address.
//!
//! # Lookup threads
//!
//! `BRRTR_DNS_THREADS` (default 4) lookup threads serve a queue of jobs. A job carries its
//! callers' deadline: a job whose callers have all given up is dropped without a lookup, so a
//! resolver that stalls on one name cannot spend later callers' budgets on work nobody waits for.
//!
//! [`DnsResolver`] also implements `may_minihttp`'s client `Resolver`, so the HTTP fetch pool
//! resolves through it ([`crate::http`]).
//!
//! # Off-coroutine calls
//!
//! A `may` socket used from a plain OS thread is driven by a worker's selector, and so is its
//! timeout. If that worker is blocked (for example in `std::thread::scope`'s `join` under a
//! handler), the call never wakes and never times out. So:
//! - [`resolve`] off a coroutine waits on nothing may-driven: it answers from the cache when the
//!   answer is fresh, otherwise runs the system resolver on a helper thread and waits for it on a
//!   std channel within the caller's timeout, falling back to the last good answer if the lookup
//!   fails or is late.
//! - [`connect`] and the `crate::http` fetches still work off a coroutine, but every such call
//!   is counted in `brrtrouter_off_coroutine_io_total{call}`, and the first one per call site
//!   kind is logged with its thread name. Prefer a coroutine, or a blocking client on OS threads.
//!
//! # Example
//!
//! ```no_run
//! use std::io::Write;
//! use std::time::Duration;
//!
//! // From a handler (a may coroutine): only this coroutine waits for the lookup and the connect.
//! let mut stream = brrtrouter::net::connect("example.com", 80, Duration::from_secs(2))?;
//! stream.write_all(b"HEAD / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")?;
//!
//! // Just the addresses, e.g. for a client that connects itself.
//! let addrs = brrtrouter::net::resolve("example.com", 443, Duration::from_millis(500))?;
//! assert!(!addrs.is_empty());
//! # Ok::<(), std::io::Error>(())
//! ```

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use may::sync::Mutex as MayMutex;
use may_minihttp::client::{Resolution, ResolutionSource, Resolver};

type Key = (String, u16);
type Answer = Result<Vec<SocketAddr>, String>;
type Reply = may::sync::mpsc::Sender<Answer>;

/// One caller waiting for a lookup; `id` lets a caller that gives up remove itself.
struct Waiter {
    id: u64,
    reply: Reply,
}

enum Entry {
    /// A lookup is queued or running. `stale` (the last good answer and when it was resolved) is
    /// served meanwhile when present.
    Pending {
        waiters: Vec<Waiter>,
        stale: Option<(Vec<SocketAddr>, Instant)>,
    },
    Ready {
        result: Answer,
        at: Instant,
    },
}

/// A lookup to run, and the time after which nobody is waiting for its answer.
struct Job {
    key: Key,
    deadline: Instant,
}

/// The process-wide resolver: a cache in front of a few OS threads that call the system resolver.
pub struct DnsResolver {
    cache: MayMutex<HashMap<Key, Entry>>,
    jobs: std::sync::Mutex<std::sync::mpsc::Sender<Job>>,
    next_waiter: AtomicU64,
    ttl: Duration,
    negative_ttl: Duration,
}

impl std::fmt::Debug for DnsResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DnsResolver")
            .field("ttl", &self.ttl)
            .field("negative_ttl", &self.negative_ttl)
            .finish_non_exhaustive()
    }
}

/// A numeric setting from the environment, or `default` when unset or unparsable.
fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// How long a background refresh (no caller waiting) may sit in the queue before it is dropped.
const REFRESH_JOB_BUDGET: Duration = Duration::from_secs(10);

static RESOLVER: OnceLock<Arc<DnsResolver>> = OnceLock::new();

/// The shared resolver, started on first use.
pub fn resolver() -> Arc<DnsResolver> {
    Arc::clone(RESOLVER.get_or_init(|| {
        DnsResolver::start(
            env_u64("BRRTR_DNS_THREADS", 4).clamp(1, 32) as usize,
            Duration::from_secs(env_u64("BRRTR_DNS_TTL_SECS", 30)),
            Duration::from_secs(env_u64("BRRTR_DNS_NEGATIVE_TTL_SECS", 2)),
        )
    }))
}

/// Resolve `host:port` without blocking a worker thread. IP literals return at once.
///
/// # Errors
/// The lookup failed, or did not finish within `timeout` (`ErrorKind::TimedOut`).
pub fn resolve(host: &str, port: u16, timeout: Duration) -> io::Result<Vec<SocketAddr>> {
    if !may::coroutine::is_coroutine() {
        note_off_coroutine(OffCoroutine::Resolve);
        return resolve_on_thread(host, port, timeout);
    }
    resolver().lookup(host, port, timeout)
}

/// [`resolve`] on a plain OS thread. Nothing here waits on a may primitive: a fresh cached answer
/// is returned as is; otherwise the system resolver runs on a helper thread and this thread waits
/// for it on a std channel, at most `timeout`. A failed or late lookup falls back to the last good
/// (expired) answer when there is one, and a successful one refreshes the cache.
#[allow(clippy::disallowed_methods, clippy::disallowed_types)] // only called off a coroutine
fn resolve_on_thread(host: &str, port: u16, timeout: Duration) -> io::Result<Vec<SocketAddr>> {
    let key = match key_or_literal(host, port) {
        Err(addr) => return Ok(vec![addr]),
        Ok(key) => key,
    };
    let r = resolver();
    let cached = r.cached(&key);
    if let Some((addrs, true)) = &cached {
        return Ok(addrs.clone());
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let job = key.clone();
    let spawned = std::thread::Builder::new()
        .name("brrtr-dns-direct".into())
        .spawn(move || {
            let result = (job.0.as_str(), job.1)
                .to_socket_addrs()
                .map(Iterator::collect::<Vec<_>>);
            let _ = tx.send(result);
        });
    let outcome = match spawned {
        Ok(_) => match rx.recv_timeout(timeout) {
            Ok(Ok(v)) if !v.is_empty() => Ok(v),
            Ok(Ok(_)) => Err(io::Error::other(format!("resolve {host}: no address"))),
            Ok(Err(e)) => Err(io::Error::other(format!("resolve {host}: {e}"))),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("resolve {host}: no answer within {timeout:?}"),
            )),
        },
        Err(e) => Err(e),
    };
    match (outcome, cached) {
        (Ok(v), _) => {
            r.store(&key, &v);
            Ok(v)
        }
        (Err(_), Some((stale, _))) => Ok(stale),
        (Err(e), None) => Err(e),
    }
}

/// The kinds of may I/O call that are counted when made off a coroutine.
#[derive(Clone, Copy, Debug)]
pub(crate) enum OffCoroutine {
    /// [`resolve`] (and [`connect`]'s lookup).
    Resolve,
    /// [`connect`].
    Connect,
    /// A `crate::http` fetch.
    HttpFetch,
}

impl OffCoroutine {
    const ALL: [Self; 3] = [Self::Resolve, Self::Connect, Self::HttpFetch];
    const fn label(self) -> &'static str {
        match self {
            Self::Resolve => "net_resolve",
            Self::Connect => "net_connect",
            Self::HttpFetch => "http_fetch",
        }
    }
}

static OFF_COROUTINE: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

/// Count a may I/O call made off a coroutine; log the first of each kind.
pub(crate) fn note_off_coroutine(kind: OffCoroutine) {
    let first = OFF_COROUTINE[kind as usize].fetch_add(1, Ordering::Relaxed) == 0;
    if first && matches!(kind, OffCoroutine::Resolve) {
        // Resolved on the calling thread (see `resolve_on_thread`): safe, counted for visibility.
        tracing::debug!(
            thread = std::thread::current().name().unwrap_or("<unnamed>"),
            "net::resolve called off a coroutine; resolved on this thread"
        );
    } else if first {
        tracing::error!(
            call = kind.label(),
            thread = std::thread::current().name().unwrap_or("<unnamed>"),
            "may I/O called off a coroutine: it depends on a may worker's selector and can hang if that worker is blocked; \
             call it from a coroutine, or use a blocking client on this OS thread (counted in brrtrouter_off_coroutine_io_total)"
        );
    }
}

/// `brrtrouter_off_coroutine_io_total{call}` in Prometheus text.
pub fn render_prometheus(out: &mut String) {
    use std::fmt::Write as _;
    let _ = writeln!(
        out,
        "# HELP brrtrouter_off_coroutine_io_total may I/O calls made from a plain OS thread rather than a coroutine"
    );
    let _ = writeln!(out, "# TYPE brrtrouter_off_coroutine_io_total counter");
    for k in OffCoroutine::ALL {
        let _ = writeln!(
            out,
            "brrtrouter_off_coroutine_io_total{{call=\"{}\"}} {}",
            k.label(),
            OFF_COROUTINE[k as usize].load(Ordering::Relaxed)
        );
    }
}

/// Like [`resolve`], but never answers from the cache: for a caller whose cached address just
/// refused a connection. The fresh answer replaces the cached one.
///
/// # Errors
/// The lookup failed, or did not finish within `timeout` (`ErrorKind::TimedOut`).
pub fn resolve_uncached(host: &str, port: u16, timeout: Duration) -> io::Result<Vec<SocketAddr>> {
    resolver().lookup_uncached(host, port, timeout)
}

/// Resolve `host:port` and connect a coroutine-aware socket within `timeout` overall. Each
/// address gets an equal share of what is left of the deadline, so one that never answers does
/// not starve the rest. `TCP_NODELAY` is set: callers write whole requests.
///
/// # Errors
/// Resolution failed, or no address accepted a connection in time.
pub fn connect(host: &str, port: u16, timeout: Duration) -> io::Result<may::net::TcpStream> {
    if !may::coroutine::is_coroutine() {
        note_off_coroutine(OffCoroutine::Connect);
    }
    let deadline = Instant::now() + timeout;
    let addrs = resolve(host, port, timeout)?;
    let mut last = None;
    let count = addrs.len();
    for (i, addr) in addrs.into_iter().enumerate() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let share = left / u32::try_from(count - i).unwrap_or(u32::MAX);
        match may::net::TcpStream::connect_timeout(&addr, share.max(Duration::from_millis(1))) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!("connect {host}:{port}: deadline exceeded"),
        )
    }))
}

/// The cache key for `host` (brackets stripped, case folded) or the address of an IP literal.
fn key_or_literal(host: &str, port: u16) -> Result<Key, SocketAddr> {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    match bare.parse::<IpAddr>() {
        Ok(ip) => Err(SocketAddr::new(ip, port)),
        Err(_) => Ok((bare.to_ascii_lowercase(), port)),
    }
}

impl DnsResolver {
    /// Start `threads` lookup threads and return the resolver they serve.
    fn start(threads: usize, ttl: Duration, negative_ttl: Duration) -> Arc<Self> {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        let this = Arc::new(Self {
            cache: MayMutex::new(HashMap::new()),
            jobs: std::sync::Mutex::new(tx),
            next_waiter: AtomicU64::new(0),
            ttl,
            negative_ttl,
        });
        let rx = Arc::new(std::sync::Mutex::new(rx));
        for i in 0..threads {
            let rx = Arc::clone(&rx);
            let me = Arc::downgrade(&this);
            let _ = std::thread::Builder::new()
                .name(format!("brrtr-dns-{i}"))
                .spawn(move || Self::lookup_thread(&rx, &me));
        }
        this
    }

    /// One lookup thread: take jobs until the resolver is dropped. An OS thread by design: this
    /// is where the blocking system resolver call belongs.
    #[allow(clippy::disallowed_methods, clippy::disallowed_types)]
    fn lookup_thread(
        rx: &std::sync::Mutex<std::sync::mpsc::Receiver<Job>>,
        me: &std::sync::Weak<Self>,
    ) {
        loop {
            let job = match rx.lock() {
                Ok(rx) => rx.recv(),
                Err(_) => return,
            };
            let Ok(Job { key, deadline }) = job else {
                return;
            };
            let Some(resolver) = me.upgrade() else {
                return;
            };
            if Instant::now() >= deadline {
                // Every caller has given up: do not spend this thread on it.
                resolver.abandon(key);
                continue;
            }
            let (host, port) = (&key.0, key.1);
            let result = (host.as_str(), port)
                .to_socket_addrs()
                .map(Iterator::collect::<Vec<_>>)
                .map_err(|e| format!("resolve {host}: {e}"))
                .and_then(|v| {
                    if v.is_empty() {
                        Err(format!("resolve {host}: no address"))
                    } else {
                        Ok(v)
                    }
                });
            resolver.finish(key, result);
        }
    }

    /// Store a lookup's answer and hand it to everyone waiting. A failed refresh keeps the last
    /// good answer, with its original timestamp so the next lookup tries again.
    fn finish(&self, key: Key, result: Answer) {
        let (waiters, answer) = {
            let Ok(mut cache) = self.cache.lock() else {
                return;
            };
            let (waiters, stale) = match cache.remove(&key) {
                Some(Entry::Pending { waiters, stale }) => (waiters, stale),
                _ => (Vec::new(), None),
            };
            let entry = match (&result, stale) {
                (Err(_), Some((addrs, at))) => Entry::Ready {
                    result: Ok(addrs),
                    at,
                },
                _ => Entry::Ready {
                    result: result.clone(),
                    at: Instant::now(),
                },
            };
            let answer = match &entry {
                Entry::Ready { result, .. } => result.clone(),
                Entry::Pending { .. } => Err("unreachable: finish stores Ready".into()),
            };
            cache.insert(key, entry);
            (waiters, answer)
        };
        for w in waiters {
            let _ = w.reply.send(answer.clone());
        }
    }

    /// A job nobody waits for any more: put back the last good answer, or forget the name.
    fn abandon(&self, key: Key) {
        let Ok(mut cache) = self.cache.lock() else {
            return;
        };
        match cache.remove(&key) {
            Some(Entry::Pending {
                stale: Some((addrs, at)),
                ..
            }) => {
                cache.insert(
                    key,
                    Entry::Ready {
                        result: Ok(addrs),
                        at,
                    },
                );
            }
            Some(Entry::Pending {
                waiters,
                stale: None,
            }) if !waiters.is_empty() => {
                // Someone arrived after the deadline was set; resolve for them after all.
                cache.insert(
                    key.clone(),
                    Entry::Pending {
                        waiters,
                        stale: None,
                    },
                );
                drop(cache);
                if !self.submit(key.clone(), Instant::now() + REFRESH_JOB_BUDGET) {
                    self.finish(key, Err("dns resolver stopped".into()));
                }
            }
            Some(other @ Entry::Ready { .. }) => {
                cache.insert(key, other);
            }
            _ => {}
        }
    }

    /// A cached good answer for `key` and whether it is still fresh (for off-coroutine callers).
    fn cached(&self, key: &Key) -> Option<(Vec<SocketAddr>, bool)> {
        let cache = self.cache.lock().ok()?;
        match cache.get(key)? {
            Entry::Ready {
                result: Ok(addrs),
                at,
            } => Some((addrs.clone(), at.elapsed() < self.ttl)),
            Entry::Pending {
                stale: Some((addrs, _)),
                ..
            } => Some((addrs.clone(), false)),
            _ => None,
        }
    }

    /// Store an answer resolved outside the lookup threads, unless a lookup for it is running.
    fn store(&self, key: &Key, addrs: &[SocketAddr]) {
        if let Ok(mut cache) = self.cache.lock() {
            if !matches!(cache.get(key), Some(Entry::Pending { .. })) {
                cache.insert(
                    key.clone(),
                    Entry::Ready {
                        result: Ok(addrs.to_vec()),
                        at: Instant::now(),
                    },
                );
            }
        }
    }

    /// Queue a lookup; `false` when the lookup threads are gone.
    fn submit(&self, key: Key, deadline: Instant) -> bool {
        self.jobs
            .lock()
            .map(|jobs| jobs.send(Job { key, deadline }).is_ok())
            .unwrap_or(false)
    }

    /// Wait for the answer to a queued lookup; on timeout, stop waiting (and stop being counted
    /// as a waiter).
    fn wait(
        &self,
        key: &Key,
        id: u64,
        rx: &may::sync::mpsc::Receiver<Answer>,
        host: &str,
        timeout: Duration,
    ) -> io::Result<Vec<SocketAddr>> {
        match rx.recv_timeout(timeout) {
            Ok(result) => result.map_err(io::Error::other),
            Err(_) => {
                if let Ok(mut cache) = self.cache.lock() {
                    if let Some(Entry::Pending { waiters, .. }) = cache.get_mut(key) {
                        waiters.retain(|w| w.id != id);
                    }
                }
                // The answer may have landed between the timeout and the unregister.
                if let Ok(result) = rx.try_recv() {
                    return result.map_err(io::Error::other);
                }
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("resolve {host}: no answer within {timeout:?}"),
                ))
            }
        }
    }

    /// Join (or start) the lookup for `key` as a waiter. Returns whether a job must be queued.
    fn enlist(
        cache: &mut HashMap<Key, Entry>,
        key: &Key,
        waiter: Waiter,
        stale: Option<(Vec<SocketAddr>, Instant)>,
    ) -> bool {
        if let Some(Entry::Pending { waiters, .. }) = cache.get_mut(key) {
            waiters.push(waiter);
            return false;
        }
        cache.insert(
            key.clone(),
            Entry::Pending {
                waiters: vec![waiter],
                stale,
            },
        );
        true
    }

    /// Resolve within `timeout`, parking only the calling coroutine.
    ///
    /// # Errors
    /// The lookup failed, or did not finish in time (`ErrorKind::TimedOut`).
    pub fn lookup(&self, host: &str, port: u16, timeout: Duration) -> io::Result<Vec<SocketAddr>> {
        let key = match key_or_literal(host, port) {
            Ok(key) => key,
            Err(addr) => return Ok(vec![addr]),
        };
        let (tx, rx) = may::sync::mpsc::channel();
        let id = self.next_waiter.fetch_add(1, Ordering::Relaxed);
        let deadline = Instant::now() + timeout;
        let submit = {
            let mut cache = self
                .cache
                .lock()
                .map_err(|_| io::Error::other("dns cache poisoned"))?;
            match cache.get(&key) {
                Some(Entry::Ready { result, at }) => {
                    let ttl = if result.is_ok() {
                        self.ttl
                    } else {
                        self.negative_ttl
                    };
                    if at.elapsed() < ttl {
                        return result.clone().map_err(io::Error::other);
                    }
                    if let Ok(addrs) = result {
                        // Expired but good: serve it while one refresh runs.
                        let addrs = addrs.clone();
                        let at = *at;
                        cache.insert(
                            key.clone(),
                            Entry::Pending {
                                waiters: Vec::new(),
                                stale: Some((addrs.clone(), at)),
                            },
                        );
                        drop(cache);
                        if !self.submit(key.clone(), Instant::now() + REFRESH_JOB_BUDGET) {
                            self.abandon(key);
                        }
                        return Ok(addrs);
                    }
                    Self::enlist(&mut cache, &key, Waiter { id, reply: tx }, None)
                }
                Some(Entry::Pending {
                    stale: Some((addrs, _)),
                    ..
                }) => return Ok(addrs.clone()),
                _ => Self::enlist(&mut cache, &key, Waiter { id, reply: tx }, None),
            }
        };
        if submit && !self.submit(key.clone(), deadline) {
            self.finish(key.clone(), Err("dns resolver stopped".into()));
        }
        self.wait(&key, id, &rx, host, timeout)
    }

    /// Resolve within `timeout` without answering from the cache (see [`resolve_uncached`]).
    ///
    /// # Errors
    /// The lookup failed, or did not finish in time (`ErrorKind::TimedOut`).
    pub fn lookup_uncached(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> io::Result<Vec<SocketAddr>> {
        let key = match key_or_literal(host, port) {
            Ok(key) => key,
            Err(addr) => return Ok(vec![addr]),
        };
        let (tx, rx) = may::sync::mpsc::channel();
        let id = self.next_waiter.fetch_add(1, Ordering::Relaxed);
        let submit = {
            let mut cache = self
                .cache
                .lock()
                .map_err(|_| io::Error::other("dns cache poisoned"))?;
            // A lookup already running is fresh enough; otherwise start one, keeping the last
            // good answer only as the fallback should this lookup fail.
            let stale = match cache.get(&key) {
                Some(Entry::Ready {
                    result: Ok(addrs),
                    at,
                }) => Some((addrs.clone(), *at)),
                _ => None,
            };
            if matches!(cache.get(&key), Some(Entry::Ready { .. })) {
                cache.remove(&key);
            }
            Self::enlist(&mut cache, &key, Waiter { id, reply: tx }, stale)
        };
        if submit && !self.submit(key.clone(), Instant::now() + timeout) {
            self.finish(key.clone(), Err("dns resolver stopped".into()));
        }
        self.wait(&key, id, &rx, host, timeout)
    }
}

impl Resolver for DnsResolver {
    fn resolve(&self, host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
        self.lookup(host, port, Duration::from_secs(5))
    }

    fn resolve_with_deadline(
        &self,
        host: &str,
        port: u16,
        deadline: Instant,
    ) -> io::Result<Resolution> {
        let left = deadline.saturating_duration_since(Instant::now());
        self.lookup(host, port, left.max(Duration::from_millis(1)))
            .map(|addresses| Resolution {
                addresses,
                source: ResolutionSource::Resolver,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Arc<DnsResolver> {
        DnsResolver::start(2, Duration::from_secs(30), Duration::from_millis(50))
    }

    #[test]
    fn off_coroutine_resolve_uses_the_system_resolver_and_is_counted() {
        let before = OFF_COROUTINE[OffCoroutine::Resolve as usize].load(Ordering::Relaxed);
        let v = resolve("localhost", 5, Duration::from_millis(1)).unwrap();
        assert!(v.iter().all(|a| a.port() == 5 && a.ip().is_loopback()));
        assert!(OFF_COROUTINE[OffCoroutine::Resolve as usize].load(Ordering::Relaxed) > before);
        let mut out = String::new();
        render_prometheus(&mut out);
        assert!(out.contains("brrtrouter_off_coroutine_io_total{call=\"net_resolve\"}"));
    }

    #[test]
    fn off_coroutine_resolve_answers_from_the_cache_and_keeps_the_last_good_answer() {
        let r = resolver();
        let fresh_key: Key = ("cached-fresh.invalid".into(), 21);
        let stale_key: Key = ("cached-stale.invalid".into(), 22);
        let fresh: Vec<SocketAddr> = vec!["10.1.2.3:21".parse().unwrap()];
        let stale: Vec<SocketAddr> = vec!["10.1.2.4:22".parse().unwrap()];
        {
            let mut cache = r.cache.lock().unwrap();
            cache.insert(
                fresh_key,
                Entry::Ready {
                    result: Ok(fresh.clone()),
                    at: Instant::now(),
                },
            );
            cache.insert(
                stale_key,
                Entry::Ready {
                    result: Ok(stale.clone()),
                    at: Instant::now() - Duration::from_secs(3600),
                },
            );
        }
        // Neither name resolves (.invalid): the fresh one is served from the cache, the expired
        // one falls back to its last good answer when the lookup fails.
        assert_eq!(
            resolve("cached-fresh.invalid", 21, Duration::from_secs(5)).unwrap(),
            fresh
        );
        assert_eq!(
            resolve("cached-stale.invalid", 22, Duration::from_secs(5)).unwrap(),
            stale
        );
        assert!(resolve("uncached.invalid", 23, Duration::from_secs(5)).is_err());
    }

    #[test]
    fn ip_literals_skip_the_resolver() {
        let r = fresh();
        let v = r.lookup("127.0.0.1", 80, Duration::from_millis(1)).unwrap();
        assert_eq!(v, vec!["127.0.0.1:80".parse().unwrap()]);
        let v = r.lookup("[::1]", 81, Duration::from_millis(1)).unwrap();
        assert_eq!(v, vec!["[::1]:81".parse().unwrap()]);
    }

    #[test]
    fn localhost_resolves_from_a_coroutine_and_is_cached() {
        let r = fresh();
        let r2 = Arc::clone(&r);
        let first = may::go!(move || r2.lookup("localhost", 8080, Duration::from_secs(5)))
            .join()
            .unwrap()
            .unwrap();
        assert!(first
            .iter()
            .all(|a| a.port() == 8080 && a.ip().is_loopback()));
        // A cached answer returns without a lookup thread.
        let started = Instant::now();
        let again = r
            .lookup("LOCALHOST", 8080, Duration::from_millis(1))
            .unwrap();
        assert_eq!(again, first);
        assert!(started.elapsed() < Duration::from_millis(50));
    }

    #[test]
    fn concurrent_lookups_share_one_resolution() {
        let r = fresh();
        let handles: Vec<_> = (0..32)
            .map(|_| {
                let r = Arc::clone(&r);
                may::go!(move || r.lookup("localhost", 9, Duration::from_secs(5)))
            })
            .collect();
        for h in handles {
            assert!(h.join().unwrap().is_ok());
        }
    }

    #[test]
    fn failures_are_errors_not_hangs() {
        let r = fresh();
        let e = r.lookup("no-such-host.invalid", 1, Duration::from_secs(10));
        assert!(e.is_err());
    }

    #[test]
    fn a_failed_refresh_keeps_the_last_good_answer() {
        let r = fresh();
        let key: Key = ("cached.invalid".into(), 7);
        let good: Vec<SocketAddr> = vec!["10.0.0.7:7".parse().unwrap()];
        let old = Instant::now() - Duration::from_secs(60);
        r.cache.lock().unwrap().insert(
            key.clone(),
            Entry::Pending {
                waiters: Vec::new(),
                stale: Some((good.clone(), old)),
            },
        );
        r.finish(key.clone(), Err("resolve cached.invalid: failed".into()));
        let cache = r.cache.lock().unwrap();
        match cache.get(&key) {
            Some(Entry::Ready { result: Ok(v), at }) => {
                assert_eq!(v, &good);
                assert_eq!(*at, old, "kept as expired, so the next lookup retries");
            }
            _ => panic!("the last good answer was not kept"),
        }
    }

    #[test]
    fn a_timed_out_caller_stops_being_a_waiter() {
        // No lookup threads: nothing ever answers.
        let (tx, _rx) = std::sync::mpsc::channel::<Job>();
        let r = Arc::new(DnsResolver {
            cache: MayMutex::new(HashMap::new()),
            jobs: std::sync::Mutex::new(tx),
            next_waiter: AtomicU64::new(0),
            ttl: Duration::from_secs(30),
            negative_ttl: Duration::from_secs(2),
        });
        let e = r
            .lookup("slow.invalid", 1, Duration::from_millis(20))
            .unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::TimedOut);
        let cache = r.cache.lock().unwrap();
        match cache.get(&("slow.invalid".into(), 1)) {
            Some(Entry::Pending { waiters, .. }) => assert!(waiters.is_empty()),
            _ => panic!("the lookup should still be pending"),
        }
    }

    #[test]
    fn an_expired_job_is_dropped_and_its_stale_answer_restored() {
        let r = fresh();
        let key: Key = ("expired.invalid".into(), 9);
        let good: Vec<SocketAddr> = vec!["10.0.0.9:9".parse().unwrap()];
        let at = Instant::now() - Duration::from_secs(60);
        r.cache.lock().unwrap().insert(
            key.clone(),
            Entry::Pending {
                waiters: Vec::new(),
                stale: Some((good.clone(), at)),
            },
        );
        r.abandon(key.clone());
        assert!(matches!(
            r.cache.lock().unwrap().get(&key),
            Some(Entry::Ready { result: Ok(_), .. })
        ));
    }

    #[test]
    fn uncached_lookups_skip_a_fresh_cache_entry() {
        let r = fresh();
        let key: Key = ("localhost".into(), 11);
        let wrong: Vec<SocketAddr> = vec!["10.9.9.9:11".parse().unwrap()];
        r.cache.lock().unwrap().insert(
            key,
            Entry::Ready {
                result: Ok(wrong.clone()),
                at: Instant::now(),
            },
        );
        let r2 = Arc::clone(&r);
        let v = may::go!(move || r2.lookup_uncached("localhost", 11, Duration::from_secs(5)))
            .join()
            .unwrap()
            .unwrap();
        assert_ne!(v, wrong);
        assert!(v.iter().all(|a| a.ip().is_loopback()));
    }

    #[test]
    fn connect_reaches_a_local_listener_by_name() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let s = may::go!(move || connect("localhost", port, Duration::from_secs(5)))
            .join()
            .unwrap();
        // localhost may list ::1 first; connect falls through to 127.0.0.1.
        assert!(s.is_ok(), "{s:?}");
        drop(listener);
    }
}
