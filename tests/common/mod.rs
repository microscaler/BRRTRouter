pub mod temp_files {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Global counter and lock for thread-safe temporary file creation
    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
    static TEMP_LOCK: Mutex<()> = Mutex::new(());

    /// Creates a temporary file with guaranteed unique name to prevent race conditions
    pub fn create_temp_spec(content: &str, ext: &str) -> PathBuf {
        let _lock = TEMP_LOCK.lock().unwrap();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "brrt_test_{}_{}_{}.{}",
            std::process::id(),
            counter,
            nanos,
            ext
        ));

        std::fs::write(&path, content).unwrap();
        path
    }

    /// Creates a temporary file with default yaml extension
    pub fn create_temp_yaml(content: &str) -> PathBuf {
        create_temp_spec(content, "yaml")
    }

    /// Creates a temporary file with json extension
    pub fn create_temp_json(content: &str) -> PathBuf {
        create_temp_spec(content, "json")
    }

    /// Cleanup temporary files (best effort)
    pub fn cleanup_temp_files(paths: &[PathBuf]) {
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub mod test_server {
    use std::sync::Once;

    /// Ensures May coroutines are configured only once
    static MAY_INIT: Once = Once::new();

    pub fn setup_may_runtime() {
        MAY_INIT.call_once(|| {
            may::config().set_stack_size(0x8000);
        });
    }
}

pub mod http {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::process::Command;
    use std::time::{Duration, Instant};

    /// Send a raw HTTP request string and return the full raw HTTP response as String.
    /// Reads headers fully, honors Content-Length for the body, and falls back
    /// to read-until-timeout when no length is provided. Includes brief retries
    /// on timeouts to avoid truncation in CI/virtualized environments.
    pub fn send_request(addr: &SocketAddr, req: &str) -> String {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(req.as_bytes()).unwrap();
        let timeout_ms: u64 = if std::env::var("ACT").is_ok() {
            1500
        } else {
            500
        };
        stream
            .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
            .unwrap();

        // Read headers first
        let mut buf = Vec::new();
        let mut header_end = None;
        for _ in 0..10 {
            let mut tmp = [0u8; 1024];
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        header_end = Some(pos + 4);
                        break;
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                Err(e) => panic!("read error: {:?}", e),
            }
        }

        let header_end = header_end.unwrap_or(buf.len());
        let headers = String::from_utf8_lossy(&buf[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|l| l.split_once(':').map(|(n, v)| (n, v)))
            .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.trim().parse::<usize>().ok());

        if let Some(clen) = content_length {
            let mut body_len = buf.len().saturating_sub(header_end);
            while body_len < clen {
                let mut tmp = [0u8; 4096];
                match stream.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        body_len += n;
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        std::thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                    Err(e) => panic!("read error: {:?}", e),
                }
            }
        } else {
            for _ in 0..10 {
                let mut tmp = [0u8; 4096];
                match stream.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        break;
                    }
                    Err(e) => panic!("read error: {:?}", e),
                }
            }
        }

        String::from_utf8_lossy(&buf).to_string()
    }

    /// Wait until an HTTP GET to `path` returns 200 OK, or timeout elapses.
    /// Returns Ok(()) on readiness; Err(message) on timeout (and best-effort logs if `container_id` is provided).
    pub fn wait_for_http_200(
        addr: &SocketAddr,
        path: &str,
        timeout: Duration,
        container_id: Option<&str>,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        let request_line = format!("GET {} HTTP/1.1\r\nHost: localhost\r\n\r\n", path);
        loop {
            if Instant::now() > deadline {
                if let Some(id) = container_id {
                    let _ = Command::new("docker")
                        .args(["logs", "--tail", "200", id])
                        .status();
                }
                return Err(format!(
                    "service did not become ready at {} within {:?}",
                    path, timeout
                ));
            }
            if let Ok(mut s) = TcpStream::connect(addr) {
                let _ = s.set_read_timeout(Some(Duration::from_millis(250)));
                let _ = s.write_all(request_line.as_bytes());
                let mut buf = [0u8; 256];
                if let Ok(n) = s.read(&mut buf) {
                    if n > 0 {
                        let head = String::from_utf8_lossy(&buf[..n]);
                        if head.starts_with("HTTP/1.1 200") || head.starts_with("HTTP/1.0 200") {
                            return Ok(());
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}
