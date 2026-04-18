use may::coroutine::JoinHandle;
use may_minihttp::{HttpServerWithHeaders, HttpService};
use std::io;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Duration;

/// Wrapper around may_minihttp's HTTP server
///
/// Provides a typed interface for starting and managing HTTP servers.
/// Uses 32 max headers (Standard) to handle modern API gateway/proxy traffic.
pub struct HttpServer<T>(pub T);

/// Handle to a running HTTP server
///
/// Provides methods for waiting until the server is ready, stopping it gracefully,
/// or joining the server thread.
pub struct ServerHandle {
    addr: SocketAddr,
    handle: JoinHandle<()>,
}

impl ServerHandle {
    /// Block until **SIGTERM** or **SIGINT**, then stop the HTTP listener and run a clean shutdown
    /// path suitable for Kubernetes (scale-down, rolling updates) and local Ctrl+C.
    ///
    /// On Unix this registers `signal-hook` handlers, waits for the first terminating signal,
    /// calls [`Self::stop`] (cancels the server coroutine and joins it), then
    /// [`crate::otel::shutdown`] to flush OpenTelemetry batch exporters.
    ///
    /// On non-Unix platforms this falls back to [`Self::join`] (wait forever unless the server
    /// thread exits on its own).
    ///
    /// # Returns
    ///
    /// Same as [`Self::join`] when the underlying server thread panicked (`Err` holds the panic
    /// payload). On Unix, registration failures fall back to [`Self::join`].
    ///
    /// # Kubernetes
    ///
    /// Ensure `terminationGracePeriodSeconds` on the Pod is large enough for in-flight requests
    /// to complete after SIGTERM (default is often 30s). The kubelet sends SIGTERM, waits for the
    /// grace period, then SIGKILL.
    pub fn run_until_shutdown(self) -> std::thread::Result<()> {
        wait_for_signal_and_shutdown(self)
    }

    /// Wait for the server to be ready to accept connections
    ///
    /// Polls the server address by attempting TCP connections until successful.
    /// Useful in tests to ensure the server is fully started before sending requests.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the server is ready
    ///
    /// # Errors
    ///
    /// Returns `TimedOut` error if the server doesn't become ready within ~250ms (50 attempts × 5ms).
    pub fn wait_ready(&self) -> io::Result<()> {
        for _ in 0..50 {
            if TcpStream::connect(self.addr).is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(5));
        }
        Err(io::Error::new(io::ErrorKind::TimedOut, "server not ready"))
    }

    /// Stop the server gracefully
    ///
    /// Cancels the server coroutine and waits for it to finish.
    /// Consumes the handle, preventing further operations.
    pub fn stop(self) {
        // SAFETY: may::CoroutineHandle::coroutine().cancel() is marked unsafe by the may runtime.
        // This is safe because:
        // - We're in Drop, so the server is shutting down
        // - The coroutine handle is valid (we're holding it)
        // - Cancellation is the intended behavior during shutdown
        unsafe {
            self.handle.coroutine().cancel();
        }
        let _ = self.handle.join();
    }

    /// Wait for the server thread to complete
    ///
    /// Blocks until the server coroutine finishes. The server will run indefinitely
    /// unless stopped externally or an error occurs.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the server thread completed successfully
    ///
    /// # Errors
    ///
    /// Returns an error if the server thread panicked.
    pub fn join(self) -> std::thread::Result<()> {
        self.handle.join()
    }
}

#[cfg(unix)]
fn wait_for_signal_and_shutdown(server: ServerHandle) -> std::thread::Result<()> {
    use signal_hook::consts::{SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = match Signals::new([SIGTERM, SIGINT]) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                target: "brrtrouter::server",
                error = %e,
                "could not register SIGTERM/SIGINT; blocking on server thread instead"
            );
            return server.join();
        }
    };

    let sig = signals.forever().next();
    drop(signals);
    if let Some(sig) = sig {
        tracing::info!(
            target: "brrtrouter::server",
            signal = sig,
            "shutdown signal received; stopping HTTP server"
        );
    }

    server.stop();
    crate::otel::shutdown();
    Ok(())
}

#[cfg(not(unix))]
fn wait_for_signal_and_shutdown(server: ServerHandle) -> std::thread::Result<()> {
    server.join()
}

impl<T: HttpService + Clone + Send + Sync + 'static> HttpServer<T> {
    /// Start the HTTP server on the given address
    ///
    /// # Arguments
    ///
    /// * `addr` - Address to bind to (e.g., `"0.0.0.0:8080"` or `"127.0.0.1:3000"`)
    ///
    /// # Returns
    ///
    /// A `ServerHandle` for managing the running server
    ///
    /// # Errors
    ///
    /// Returns an error if the address is invalid or the port cannot be bound.
    pub fn start<A: ToSocketAddrs>(self, addr: A) -> io::Result<ServerHandle> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid address"))?;
        // Use HttpServerWithHeaders<_, 32> to handle modern API gateway/proxy traffic
        let handle = HttpServerWithHeaders::<_, 32>(self.0).start(addr)?;
        Ok(ServerHandle { addr, handle })
    }
}
