use may_minihttp::Response;
use serde_json::Value;

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

use std::collections::HashMap;

/// Write a handler response to the HTTP response object
///
/// Serializes the response body as JSON or plain text and sets appropriate headers.
/// Handles Server-Sent Events (SSE) responses with special content type.
///
/// # Arguments
///
/// * `res` - HTTP response object to write to
/// * `status` - HTTP status code (e.g., 200, 404, 500)
/// * `body` - Response body as JSON value
/// * `is_sse` - Whether this is a Server-Sent Events response
/// * `headers` - Additional HTTP headers to include
pub fn write_handler_response(
    res: &mut Response,
    status: u16,
    body: Value,
    is_sse: bool,
    headers: &HashMap<String, String>,
) {
    let reason = status_reason(status);
    res.status_code(status as usize, reason);
    for (k, v) in headers {
        let header_value = format!("{k}: {v}").into_boxed_str();
        res.header(Box::leak(header_value));
    }
    match body {
        Value::String(s) => {
            if is_sse {
                res.header("Content-Type: text/event-stream");
            } else {
                res.header("Content-Type: text/plain");
            }
            res.body_vec(s.into_bytes());
        }
        other => {
            res.header("Content-Type: application/json");
            // Serialize to JSON - if this fails, use fallback error response
            match serde_json::to_vec(&other) {
                Ok(json_bytes) => res.body_vec(json_bytes),
                Err(e) => {
                    // Serialization failed - send error message as plain text
                    res.status_code(500, "Internal Server Error");
                    res.header("Content-Type: text/plain");
                    res.body_vec(format!("Failed to serialize response: {}", e).into_bytes());
                }
            }
        }
    }
}

/// Write a JSON error response to the HTTP response object
///
/// Used for validation errors, authentication failures, and other error conditions.
/// Always sets content-type to `application/json`.
///
/// # Arguments
///
/// * `res` - HTTP response object to write to
/// * `status` - HTTP status code (typically 400-599)
/// * `body` - Error response body as JSON (usually includes "error" field)
pub fn write_json_error(res: &mut Response, status: u16, body: Value) {
    let reason = status_reason(status);
    res.status_code(status as usize, reason);
    res.header("Content-Type: application/json");
    res.body_vec(body.to_string().into_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use may_minihttp::{HttpServer, HttpService, Request, Response};
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;

    #[derive(Clone)]
    struct HandlerService;

    impl HttpService for HandlerService {
        fn call(&mut self, _req: Request, res: &mut Response) -> std::io::Result<()> {
            let mut headers = HashMap::new();
            headers.insert("X-Test".to_string(), "foo".to_string());
            write_handler_response(res, 201, serde_json::json!({"ok": true}), false, &headers);
            Ok(())
        }
    }

    #[derive(Clone)]
    struct ErrorService;

    impl HttpService for ErrorService {
        fn call(&mut self, _req: Request, res: &mut Response) -> std::io::Result<()> {
            write_json_error(res, 404, serde_json::json!({"error": "nope"}));
            Ok(())
        }
    }

    fn send_request(addr: &std::net::SocketAddr, req: &str) -> String {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(req.as_bytes()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
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

    fn parse_parts(resp: &str) -> (u16, String, String) {
        let mut parts = resp.split("\r\n\r\n");
        let headers = parts.next().unwrap_or("");
        let body = parts.next().unwrap_or("").to_string();
        let mut status = 0;
        let mut ct = String::new();
        let mut x_test = false;
        for line in headers.lines() {
            if line.starts_with("HTTP/1.1") {
                status = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("0")
                    .parse()
                    .unwrap();
            } else if let Some((n, v)) = line.split_once(':') {
                if n.eq_ignore_ascii_case("content-type") {
                    ct = v.trim().to_string();
                } else if n.eq_ignore_ascii_case("x-test") {
                    x_test = true;
                }
            }
        }
        let info = if x_test { format!("{}|X-Test", ct) } else { ct };
        (status, info, body)
    }

    #[test]
    fn test_status_reason() {
        assert_eq!(status_reason(200), "OK");
        assert_eq!(status_reason(404), "Not Found");
    }

    #[test]
    fn test_write_handler_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(HandlerService).start(addr).unwrap();
        let resp = send_request(&addr, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        unsafe { handle.coroutine().cancel() };
        let (status, info, body) = parse_parts(&resp);
        assert_eq!(status, 201);
        assert!(info.starts_with("application/json"));
        assert!(info.contains("X-Test"));
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn test_write_json_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(ErrorService).start(addr).unwrap();
        let resp = send_request(&addr, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        unsafe { handle.coroutine().cancel() };
        let (status, ct, body) = parse_parts(&resp);
        assert_eq!(status, 404);
        assert_eq!(ct, "application/json");
        assert_eq!(body, "{\"error\":\"nope\"}");
    }
}
