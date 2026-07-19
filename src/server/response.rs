use crate::dispatcher::HeaderVec;
use may_minihttp::Response;
use serde_json::Value;

/// Whether an HTTP status permits a response body.
///
/// RFC 9110 forbids content on informational responses, 204, and 304; 205 also
/// requires a zero-length response. Handlers may still use a JSON value as an
/// internal sentinel, but it must not be validated or written on the wire.
#[inline]
#[must_use]
pub(crate) fn response_status_allows_body(status: u16) -> bool {
    !(100..200).contains(&status) && !matches!(status, 204 | 205 | 304)
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

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
/// * `headers` - Additional HTTP headers to include (SmallVec for stack allocation)
pub fn write_handler_response(
    res: &mut Response,
    status: u16,
    body: Value,
    is_sse: bool,
    headers: &HeaderVec,
) {
    let reason = status_reason(status);
    res.status_code(status as usize, reason);
    // Owned headers are freed with the `Response` — `may_minihttp` accepts
    // `String` via `IntoResponseHeader` on our fork, so no `Box::leak`.
    //
    // Track Content-Type from the handler/OpenAPI map so we do not emit a
    // second casing (`content-type` + `Content-Type`). Nginx treats that as a
    // duplicate header and can return 502 to the browser.
    let mut has_content_type = false;
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("content-length") {
            continue;
        }
        if k.eq_ignore_ascii_case("content-type") {
            has_content_type = true;
        }
        res.header(format!("{k}: {v}"));
    }
    if !response_status_allows_body(status) {
        return;
    }
    match body {
        Value::String(s) => {
            if !has_content_type {
                if is_sse {
                    res.header("Content-Type: text/event-stream");
                } else {
                    res.header("Content-Type: text/plain");
                }
            }
            res.body_vec(s.into_bytes());
        }
        other => {
            match serde_json::to_vec(&other) {
                Ok(json_bytes) => {
                    if !has_content_type {
                        res.header("Content-Type: application/json");
                    }
                    res.body_vec(json_bytes);
                }
                Err(e) => {
                    res.status_code(500, "Internal Server Error");
                    if !has_content_type {
                        res.header("Content-Type: text/plain");
                    }
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
    // Direct Vec<u8> — avoids the intermediate `String` allocation from
    // `to_string().into_bytes()`.
    match serde_json::to_vec(&body) {
        Ok(bytes) => res.body_vec(bytes),
        Err(_) => res.body_vec(br#"{"error":"serialization failure"}"#.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use may_minihttp::{HttpServer, HttpService, Request, Response};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Clone)]
    struct HandlerService;

    impl HttpService for HandlerService {
        fn call(&mut self, _req: Request, res: &mut Response) -> std::io::Result<()> {
            let mut headers: HeaderVec = HeaderVec::new();
            // JSF P2: Use Arc::from for header names
            headers.push((Arc::from("x-test"), "foo".to_string()));
            write_handler_response(res, 201, serde_json::json!({"ok": true}), false, &headers);
            Ok(())
        }
    }

    /// Simulates the OpenAPI/proxy path that already injects `content-type`
    /// (lowercase) before `write_handler_response` adds a default.
    #[derive(Clone)]
    struct PrefixedContentTypeService;

    impl HttpService for PrefixedContentTypeService {
        fn call(&mut self, _req: Request, res: &mut Response) -> std::io::Result<()> {
            let mut headers: HeaderVec = HeaderVec::new();
            headers.push((Arc::from("content-type"), "application/json".to_string()));
            write_handler_response(res, 200, serde_json::json!({"ok": true}), false, &headers);
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

    #[derive(Clone)]
    struct ForbiddenJsonService;

    impl HttpService for ForbiddenJsonService {
        fn call(&mut self, _req: Request, res: &mut Response) -> std::io::Result<()> {
            write_handler_response(
                res,
                403,
                serde_json::json!({"error": "Origin not allowed by CORS policy"}),
                false,
                &HeaderVec::new(),
            );
            Ok(())
        }
    }

    /// Mirrors `CorsMiddleware` + `AppService`: `HandlerResponse::error` then `write_handler_response`.
    #[derive(Clone)]
    struct CorsForbiddenViaHandlerResponse;

    impl HttpService for CorsForbiddenViaHandlerResponse {
        fn call(&mut self, _req: Request, res: &mut Response) -> std::io::Result<()> {
            let hr =
                crate::dispatcher::HandlerResponse::error(403, "Origin not allowed by CORS policy");
            write_handler_response(res, hr.status, hr.body, false, &hr.headers);
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
                // In test code, panicking on read errors is acceptable for test failure clarity
                #[cfg(test)]
                #[allow(clippy::panic)]
                // Test code: panicking on I/O errors is acceptable
                Err(e) => panic!("read error: {:?}", e),
                #[cfg(not(test))]
                Err(e) => {
                    // In production, this should return an error, but this function is only used in tests
                    panic!("read error: {:?}", e);
                }
            }
        }
        let header_end = header_end.unwrap_or(buf.len());
        let headers = String::from_utf8_lossy(&buf[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|l| l.split_once(':'))
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
                    // In test code, panicking on read errors is acceptable for test failure clarity
                    #[cfg(test)]
                    #[allow(clippy::panic)]
                    // Test code: panicking on I/O errors is acceptable
                    Err(e) => panic!("read error: {:?}", e),
                    #[cfg(not(test))]
                    Err(e) => {
                        // In production, this should return an error, but this function is only used in tests
                        panic!("read error: {:?}", e);
                    }
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
                    // In test code, panicking on read errors is acceptable for test failure clarity
                    #[cfg(test)]
                    #[allow(clippy::panic)]
                    // Test code: panicking on I/O errors is acceptable
                    Err(e) => panic!("read error: {:?}", e),
                    #[cfg(not(test))]
                    Err(e) => {
                        // In production, this should return an error, but this function is only used in tests
                        panic!("read error: {:?}", e);
                    }
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
        assert_eq!(status_reason(403), "Forbidden");
        assert_eq!(status_reason(404), "Not Found");
    }

    #[test]
    fn test_write_handler_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(HandlerService).start(addr).unwrap();
        let resp = send_request(&addr, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        // SAFETY: may::CoroutineHandle::coroutine().cancel() is marked unsafe by the may runtime.
        // Safe in tests: coroutine handle is valid, cancellation is for test cleanup
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
        // SAFETY: may::CoroutineHandle::coroutine().cancel() is marked unsafe by the may runtime.
        // Safe in tests: coroutine handle is valid, cancellation is for test cleanup
        unsafe { handle.coroutine().cancel() };
        let (status, ct, body) = parse_parts(&resp);
        assert_eq!(status, 404);
        assert_eq!(ct, "application/json");
        assert_eq!(body, "{\"error\":\"nope\"}");
    }

    #[derive(Clone)]
    struct DuplicateContentLengthService;

    impl HttpService for DuplicateContentLengthService {
        fn call(&mut self, _req: Request, res: &mut Response) -> std::io::Result<()> {
            let mut headers: HeaderVec = HeaderVec::new();
            headers.push((Arc::from("content-length"), "999".to_string()));
            write_handler_response(res, 200, serde_json::json!({"ok": true}), false, &headers);
            Ok(())
        }
    }

    fn count_header(resp: &str, name: &str) -> usize {
        resp.lines()
            .filter(|line| {
                line.split_once(':')
                    .map(|(n, _)| n.eq_ignore_ascii_case(name))
                    .unwrap_or(false)
            })
            .count()
    }

    #[test]
    fn test_write_handler_response_ignores_incoming_content_length() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(DuplicateContentLengthService)
            .start(addr)
            .unwrap();
        let resp = send_request(&addr, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        unsafe { handle.coroutine().cancel() };
        assert_eq!(
            count_header(&resp, "content-length"),
            1,
            "response must not duplicate Content-Length"
        );
        let (_, _, body) = parse_parts(&resp);
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn test_write_handler_response_does_not_duplicate_content_type() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(PrefixedContentTypeService).start(addr).unwrap();
        let resp = send_request(&addr, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        unsafe { handle.coroutine().cancel() };
        assert_eq!(
            count_header(&resp, "content-type"),
            1,
            "duplicate Content-Type (any casing) breaks nginx reverse proxies with 502: {resp}"
        );
        let (status, ct, body) = parse_parts(&resp);
        assert_eq!(status, 200);
        assert_eq!(ct, "application/json");
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn test_write_handler_response_403_uses_forbidden_not_ok() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(ForbiddenJsonService).start(addr).unwrap();
        let resp = send_request(&addr, "POST /form HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 11\r\n\r\nfoo=bar&baz=1");
        unsafe { handle.coroutine().cancel() };
        let first = resp.lines().next().unwrap_or("");
        assert!(
            first.contains("403 Forbidden"),
            "status line must use Forbidden for 403, got {first:?}"
        );
        assert!(
            !first.contains("403 OK"),
            "must not emit 403 with OK reason phrase (regression guard): {first:?}"
        );
        let (_status, _ct, body) = parse_parts(&resp);
        assert_eq!(
            body, "{\"error\":\"Origin not allowed by CORS policy\"}",
            "JSON body must be an object, not null"
        );
    }

    #[test]
    fn test_cors_handler_response_error_round_trip_not_null() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(CorsForbiddenViaHandlerResponse)
            .start(addr)
            .unwrap();
        let resp = send_request(
            &addr,
            "POST /webhooks HTTP/1.1\r\nHost: localhost\r\nOrigin: https://evil.example\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
        );
        unsafe { handle.coroutine().cancel() };
        let first = resp.lines().next().unwrap_or("");
        assert!(
            first.contains("403 Forbidden"),
            "expected 403 Forbidden status line, got {first:?}"
        );
        let (_status, _ct, body) = parse_parts(&resp);
        assert_ne!(
            body,
            "null",
            "CORS HandlerResponse::error must not write bare JSON null (regression: POST /webhooks UI)"
        );
        assert!(body.contains("Origin not allowed"), "body={body:?}");
    }
}
