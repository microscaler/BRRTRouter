use may::coroutine::JoinHandle;
use may_minihttp::{HttpServer as MiniHttpServer, HttpService};
use std::io;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Duration;

/// Wrapper around may_minihttp's HTTP server
///
/// Provides a typed interface for starting and managing HTTP servers.
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
    pub fn wait_ready(&self) -> io::Result<()> {
        for _ in 0..50 {
            if TcpStream::connect(self.addr).is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(5));
        }
        Err(io::Error::new(io::ErrorKind::TimedOut, "server not ready"))
    }

    pub fn stop(self) {
        unsafe {
            self.handle.coroutine().cancel();
        }
        let _ = self.handle.join();
    }

    pub fn join(self) -> std::thread::Result<()> {
        self.handle.join()
    }
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
        let handle = MiniHttpServer(self.0).start(addr)?;
        Ok(ServerHandle { addr, handle })
    }
}
