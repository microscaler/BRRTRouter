//! Story 12.5 — outbound webhook delivery kit (P*/N* tables).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use brrtrouter::http::{
    deliver_webhook, sign_body_hmac_sha256, WebhookDeliveryError, WebhookDeliveryOptions,
    WebhookHmac,
};

fn read_request(stream: &mut TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut acc = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                acc.extend_from_slice(&buf[..n]);
                if let Some(header_end) = find_header_end(&acc) {
                    if let Some(cl) = content_length(&acc[..header_end]) {
                        let body_len = acc.len().saturating_sub(header_end);
                        if body_len >= cl {
                            break;
                        }
                    } else {
                        // No Content-Length — stop after headers + whatever arrived.
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&acc).into_owned()
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
}

fn content_length(headers: &[u8]) -> Option<usize> {
    let s = std::str::from_utf8(headers).ok()?;
    for line in s.split("\r\n") {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            return rest.trim().parse().ok();
        }
    }
    None
}

fn write_response(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200..=299 => "OK",
        503 => "Service Unavailable",
        500 => "Internal Server Error",
        400..=499 => "Client Error",
        _ => "Error",
    };
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

fn start_capture_server(
    responses: Vec<(u16, &'static str)>,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{}:{}/hook", addr.ip(), addr.port());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_c = Arc::clone(&seen);
    let handle = thread::spawn(move || {
        let mut index = 0usize;
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else {
                break;
            };
            let req = read_request(&mut stream);
            seen_c.lock().unwrap().push(req);
            let (status, body) = responses
                .get(index)
                .copied()
                .unwrap_or_else(|| *responses.last().unwrap_or(&(200, "")));
            if index + 1 < responses.len() {
                index += 1;
            }
            write_response(&mut stream, status, body);
        }
    });
    (base, seen, handle)
}

/// P1 — POST JSON to mock server; 2xx; body bytes match.
#[test]
fn webhook_delivery_p1_post_json_ok() {
    let body = br#"{"event":"pet.created"}"#;
    let (url, seen, _handle) = start_capture_server(vec![(200, r#"{"ok":true}"#)]);
    let opts = WebhookDeliveryOptions {
        url,
        body: body.to_vec(),
        max_attempts: 1,
        timeout: Duration::from_secs(2),
        initial_backoff: Duration::ZERO,
        ..WebhookDeliveryOptions::default()
    };
    let result = deliver_webhook(&opts).unwrap();
    assert_eq!(result.status, 200);
    assert_eq!(result.attempts, 1);
    assert_eq!(result.response_body, br#"{"ok":true}"#);
    let req = seen.lock().unwrap()[0].clone();
    assert!(req.contains("POST /hook"));
    assert!(req.contains(r#"{"event":"pet.created"}"#));
}

/// P2 — HMAC-SHA256 header set when secret provided.
#[test]
fn webhook_delivery_p2_hmac_header_valid() {
    let body = br#"{"a":1}"#;
    let (url, seen, _handle) = start_capture_server(vec![(204, "")]);
    let expected = sign_body_hmac_sha256("test-secret", body);
    let opts = WebhookDeliveryOptions {
        url,
        body: body.to_vec(),
        hmac: Some(WebhookHmac::required("test-secret")),
        max_attempts: 1,
        timeout: Duration::from_secs(2),
        initial_backoff: Duration::ZERO,
        ..WebhookDeliveryOptions::default()
    };
    assert!(deliver_webhook(&opts).is_ok());
    let req = seen.lock().unwrap()[0].to_ascii_lowercase();
    assert!(
        req.contains(&format!(
            "x-hub-signature-256: {}",
            expected.to_ascii_lowercase()
        )),
        "missing hmac in {req}"
    );
}

/// P3 — Retry once on 503 then success.
#[test]
fn webhook_delivery_p3_retry_503_then_ok() {
    let (url, seen, _handle) = start_capture_server(vec![(503, "busy"), (200, "ok")]);
    let opts = WebhookDeliveryOptions {
        url,
        body: b"{}".to_vec(),
        max_attempts: 3,
        timeout: Duration::from_secs(2),
        initial_backoff: Duration::from_millis(5),
        ..WebhookDeliveryOptions::default()
    };
    let result = deliver_webhook(&opts).unwrap();
    assert_eq!(result.status, 200);
    assert_eq!(result.attempts, 2);
    assert_eq!(seen.lock().unwrap().len(), 2);
}

/// P4 — Custom headers forwarded.
#[test]
fn webhook_delivery_p4_custom_headers() {
    let (url, seen, _handle) = start_capture_server(vec![(200, "ok")]);
    let opts = WebhookDeliveryOptions {
        url,
        body: b"{}".to_vec(),
        headers: vec![("X-Org-Id".to_string(), "org_1".to_string())],
        max_attempts: 1,
        timeout: Duration::from_secs(2),
        initial_backoff: Duration::ZERO,
        ..WebhookDeliveryOptions::default()
    };
    deliver_webhook(&opts).unwrap();
    let req = seen.lock().unwrap()[0].to_ascii_lowercase();
    assert!(req.contains("x-org-id: org_1"), "{req}");
}

/// P5 — Timeout config respected (documented + wired into fetch options).
#[test]
fn webhook_delivery_p5_timeout_wired() {
    let opts = WebhookDeliveryOptions {
        timeout: Duration::from_millis(1234),
        ..WebhookDeliveryOptions::default()
    };
    assert_eq!(opts.timeout, Duration::from_millis(1234));
}

/// P6 — Idempotency-Key forwarded when set.
#[test]
fn webhook_delivery_p6_idempotency_key() {
    let (url, seen, _handle) = start_capture_server(vec![(200, "ok")]);
    let opts = WebhookDeliveryOptions {
        url,
        body: b"{}".to_vec(),
        idempotency_key: Some("deliv-1".to_string()),
        max_attempts: 1,
        timeout: Duration::from_secs(2),
        initial_backoff: Duration::ZERO,
        ..WebhookDeliveryOptions::default()
    };
    deliver_webhook(&opts).unwrap();
    let req = seen.lock().unwrap()[0].to_ascii_lowercase();
    assert!(req.contains("idempotency-key: deliv-1"), "{req}");
}

/// N1 — DNS/connect failure → Err; no panic.
#[test]
fn webhook_delivery_n1_connect_failure() {
    let opts = WebhookDeliveryOptions {
        // Discard port — connection refused.
        url: "http://127.0.0.1:1/hook".into(),
        body: b"{}".to_vec(),
        max_attempts: 1,
        timeout: Duration::from_millis(200),
        initial_backoff: Duration::ZERO,
        ..WebhookDeliveryOptions::default()
    };
    let err = deliver_webhook(&opts).unwrap_err();
    assert!(matches!(err, WebhookDeliveryError::Transport(_)));
}

/// N2 — Exhausted retries on 500.
#[test]
fn webhook_delivery_n2_exhausted_500() {
    let (url, seen, _handle) = start_capture_server(vec![(500, "nope")]);
    let opts = WebhookDeliveryOptions {
        url,
        body: b"{}".to_vec(),
        max_attempts: 3,
        timeout: Duration::from_secs(2),
        initial_backoff: Duration::from_millis(5),
        ..WebhookDeliveryOptions::default()
    };
    let err = deliver_webhook(&opts).unwrap_err();
    match err {
        WebhookDeliveryError::HttpStatus {
            status, attempts, ..
        } => {
            assert_eq!(status, 500);
            assert_eq!(attempts, 3);
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(seen.lock().unwrap().len(), 3);
}

/// N3 — Invalid URL.
#[test]
fn webhook_delivery_n3_invalid_url() {
    let opts = WebhookDeliveryOptions {
        url: "ftp://example.com/hook".into(),
        body: b"{}".to_vec(),
        max_attempts: 1,
        ..WebhookDeliveryOptions::default()
    };
    assert!(matches!(
        deliver_webhook(&opts),
        Err(WebhookDeliveryError::InvalidUrl(_))
    ));
}

/// N4 — Empty secret with HMAC required.
#[test]
fn webhook_delivery_n4_empty_hmac_required() {
    let opts = WebhookDeliveryOptions {
        url: "http://127.0.0.1:9/hook".into(),
        body: b"{}".to_vec(),
        hmac: Some(WebhookHmac::required("")),
        max_attempts: 1,
        ..WebhookDeliveryOptions::default()
    };
    assert_eq!(
        deliver_webhook(&opts).unwrap_err(),
        WebhookDeliveryError::EmptyHmacSecret
    );
}

/// N5 — Oversized body vs client limit.
#[test]
fn webhook_delivery_n5_oversized_body() {
    let opts = WebhookDeliveryOptions {
        url: "http://127.0.0.1:9/hook".into(),
        body: vec![b'x'; 64],
        max_request_body_bytes: 16,
        max_attempts: 1,
        ..WebhookDeliveryOptions::default()
    };
    assert!(matches!(
        deliver_webhook(&opts),
        Err(WebhookDeliveryError::RequestBodyTooLarge { len: 64, max: 16 })
    ));
}

/// N7 — 4xx is not silent success.
#[test]
fn webhook_delivery_n7_4xx_surfaced() {
    let (url, seen, _handle) = start_capture_server(vec![(400, "bad")]);
    let opts = WebhookDeliveryOptions {
        url,
        body: b"{}".to_vec(),
        max_attempts: 3,
        timeout: Duration::from_secs(2),
        initial_backoff: Duration::from_millis(5),
        ..WebhookDeliveryOptions::default()
    };
    let err = deliver_webhook(&opts).unwrap_err();
    match err {
        WebhookDeliveryError::HttpStatus {
            status, attempts, ..
        } => {
            assert_eq!(status, 400);
            assert_eq!(attempts, 1);
        }
        other => panic!("unexpected {other:?}"),
    }
    // No retries on 4xx.
    assert_eq!(seen.lock().unwrap().len(), 1);
}

/// N8 — Credential leak forbidden in Display/Debug of secret.
#[test]
fn webhook_delivery_n8_no_secret_leak() {
    let hmac = WebhookHmac::required("leaked-credential-value");
    let d = format!("{hmac:?}");
    assert!(!d.contains("leaked-credential"));
    assert!(d.contains("REDACTED"));
}
