use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse},
    router::Router,
    server::AppService,
    spec::{ResponseSpec, RouteMeta},
};
use http::Method;
use brrtrouter::server::{HttpServer, ServerHandle};
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

fn send_request(addr: &std::net::SocketAddr, req: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(req.as_bytes()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut buf = Vec::new();
    loop {
        let mut tmp = [0u8; 1024];
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                break
            }
            Err(e) => panic!("read error: {:?}", e),
        }
    }
    String::from_utf8_lossy(&buf).to_string()
}

fn parse_parts(resp: &str) -> (u16, String) {
    let mut parts = resp.split("\r\n\r\n");
    let headers = parts.next().unwrap_or("");
    let mut status = 0;
    let mut ct = String::new();
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
            }
        }
    }
    (status, ct)
}

#[test]
fn test_select_content_type_from_spec() {
    may::config().set_stack_size(0x8000);
    let responses = {
        let mut m = HashMap::new();
        let mut inner = HashMap::new();
        inner.insert(
            "text/plain".to_string(),
            ResponseSpec {
                schema: None,
                example: None,
            },
        );
        m.insert(201u16, inner);
        m
    };
    let route = RouteMeta {
        method: Method::POST,
        path_pattern: "/resp".to_string(),
        handler_name: "h".to_string(),
        parameters: vec![],
        request_schema: None,
        response_schema: None,
        example: None,
        responses,
        security: vec![],
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
    };
    let router = Arc::new(RwLock::new(Router::new(vec![route.clone()])));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("h", |_req: HandlerRequest| {
            let resp = HandlerResponse {
                status: 201,
                headers: HashMap::new(),
                body: json!("ok"),
            };
            let _ = _req.reply_tx.send(resp);
        });
    }
    let service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        HashMap::new(),
        PathBuf::new(),
        None,
        None,
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    let resp = send_request(&addr, "POST /resp HTTP/1.1\r\nHost: x\r\n\r\n");
    handle.stop();
    let (status, ct) = parse_parts(&resp);
    assert_eq!(status, 201);
    assert_eq!(ct, "text/plain");
}
