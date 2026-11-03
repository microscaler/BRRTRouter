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
//! Resolved names are cached (`BRRTR_DNS_TTL_SECS`, default 30) and refreshed in the background
//! once stale: a stale entry is served while the refresh runs, so a hot path pays for a lookup at
//! most once per name. Concurrent lookups of one name share a single resolution. Failures are
//! cached for `BRRTR_DNS_NEGATIVE_TTL_SECS` (default 2). `BRRTR_DNS_THREADS` (default 2) sizes
//! the lookup threads.
//!
//! [`DnsResolver`] also implements `may_minihttp`'s client `Resolver`, so the HTTP fetch pool
//! resolves through it ([`crate::http`]).

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use may::sync::Mutex as MayMutex;
use may_minihttp::client::{Resolution, ResolutionSource, Resolver};

type Key = (String, u16);
type Reply = may::sync::mpsc::Sender<Result<Vec<SocketAddr>, String>>;

enum Entry {
    /// A lookup is running; these callers wait for it. `stale` is served meanwhile when present.
    Pending {
        waiters: Vec<Reply>,
        stale: Option<Vec<SocketAddr>>,
    },
    Ready {
        result: Result<Vec<SocketAddr>, String>,
        at: Instant,
    },
}

/// The process-wide resolver: a cache in front of a few OS threads that call the system resolver.
pub struct DnsResolver {
    cache: MayMutex<HashMap<Key, Entry>>,
    jobs: std::sync::Mutex<std::sync::mpsc::Sender<Key>>,
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

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

static RESOLVER: OnceLock<Arc<DnsResolver>> = OnceLock::new();

/// The shared resolver, started on first use.
pub fn resolver() -> Arc<DnsResolver> {
    Arc::clone(RESOLVER.get_or_init(|| {
        DnsResolver::start(
            env_u64("BRRTR_DNS_THREADS", 2).clamp(1, 16) as usize,
            Duration::from_secs(env_u64("BRRTR_DNS_TTL_SECS", 30)),
            Duration::from_secs(env_u64("BRRTR_DNS_NEGATIVE_TTL_SECS", 2)),
        )
    }))
}

/// Resolve `host:port` without blocking a worker thread. IP literals return at once.
///
/// # Errors
/// The lookup failed, or did not finish within `timeout`.
pub fn resolve(host: &str, port: u16, timeout: Duration) -> io::Result<Vec<SocketAddr>> {
    resolver().lookup(host, port, timeout)
}

/// Resolve `host:port` and connect a coroutine-aware socket, trying each address within
/// `timeout` overall. `TCP_NODELAY` is set: callers write whole requests.
///
/// # Errors
/// Resolution failed, or no address accepted a connection in time.
pub fn connect(host: &str, port: u16, timeout: Duration) -> io::Result<may::net::TcpStream> {
    let deadline = Instant::now() + timeout;
    let addrs = resolve(host, port, timeout)?;
    let mut last = None;
    for addr in addrs {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match may::net::TcpStream::connect_timeout(&addr, left) {
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

impl DnsResolver {
    fn start(threads: usize, ttl: Duration, negative_ttl: Duration) -> Arc<Self> {
        let (tx, rx) = std::sync::mpsc::channel::<Key>();
        let this = Arc::new(Self {
            cache: MayMutex::new(HashMap::new()),
            jobs: std::sync::Mutex::new(tx),
            ttl,
            negative_ttl,
        });
        let rx = Arc::new(std::sync::Mutex::new(rx));
        for i in 0..threads {
            let rx = Arc::clone(&rx);
            let me = Arc::downgrade(&this);
            let _ = std::thread::Builder::new()
                .name(format!("brrtr-dns-{i}"))
                .spawn(
                    #[allow(clippy::disallowed_methods, clippy::disallowed_types)]
                    // OS threads by design: this is where the blocking resolver call belongs.
                    move || loop {
                        let job = match rx.lock() {
                            Ok(rx) => rx.recv(),
                            Err(_) => return,
                        };
                        let Ok((host, port)) = job else { return };
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
                        let Some(me) = me.upgrade() else { return };
                        me.finish((host, port), result);
                    },
                );
        }
        this
    }

    fn finish(&self, key: Key, result: Result<Vec<SocketAddr>, String>) {
        let waiters = {
            let Ok(mut cache) = self.cache.lock() else {
                return;
            };
            let previous = cache.insert(
                key,
                Entry::Ready {
                    result: result.clone(),
                    at: Instant::now(),
                },
            );
            match previous {
                Some(Entry::Pending { waiters, .. }) => waiters,
                _ => Vec::new(),
            }
        };
        for w in waiters {
            let _ = w.send(result.clone());
        }
    }

    fn submit(&self, key: Key) -> bool {
        self.jobs
            .lock()
            .map(|jobs| jobs.send(key).is_ok())
            .unwrap_or(false)
    }

    /// Resolve within `timeout`, parking only the calling coroutine.
    ///
    /// # Errors
    /// The lookup failed, or did not finish in time.
    pub fn lookup(&self, host: &str, port: u16, timeout: Duration) -> io::Result<Vec<SocketAddr>> {
        let bare = host.trim_start_matches('[').trim_end_matches(']');
        if let Ok(ip) = bare.parse::<IpAddr>() {
            return Ok(vec![SocketAddr::new(ip, port)]);
        }
        let key: Key = (bare.to_ascii_lowercase(), port);
        let (tx, rx) = may::sync::mpsc::channel();
        let mut submit = false;
        {
            let mut cache = self
                .cache
                .lock()
                .map_err(|_| io::Error::other("dns cache poisoned"))?;
            match cache.get_mut(&key) {
                Some(Entry::Ready { result, at }) => {
                    let ttl = if result.is_ok() {
                        self.ttl
                    } else {
                        self.negative_ttl
                    };
                    if at.elapsed() < ttl {
                        return result.clone().map_err(io::Error::other);
                    }
                    // Stale: serve the last good answer while one refresh runs.
                    let stale = result.as_ref().ok().cloned();
                    if let Some(stale) = stale {
                        cache.insert(
                            key.clone(),
                            Entry::Pending {
                                waiters: Vec::new(),
                                stale: Some(stale.clone()),
                            },
                        );
                        drop(cache);
                        if !self.submit(key.clone()) {
                            self.finish(key, Ok(stale.clone()));
                        }
                        return Ok(stale);
                    }
                    cache.insert(
                        key.clone(),
                        Entry::Pending {
                            waiters: vec![tx],
                            stale: None,
                        },
                    );
                    submit = true;
                }
                Some(Entry::Pending { waiters, stale }) => {
                    if let Some(stale) = stale {
                        return Ok(stale.clone());
                    }
                    waiters.push(tx);
                }
                None => {
                    cache.insert(
                        key.clone(),
                        Entry::Pending {
                            waiters: vec![tx],
                            stale: None,
                        },
                    );
                    submit = true;
                }
            }
        }
        if submit && !self.submit(key.clone()) {
            self.finish(key, Err("dns resolver stopped".into()));
        }
        match rx.recv_timeout(timeout) {
            Ok(result) => result.map_err(io::Error::other),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("resolve {host}: no answer within {timeout:?}"),
            )),
        }
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
