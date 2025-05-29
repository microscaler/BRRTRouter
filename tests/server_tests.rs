use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse},
    router::Router,
    server::AppService,
    spec::RouteMeta,
};
use http::Method;
use may_minihttp::HttpServer;
use pet_store::registry;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;
use std::path::PathBuf;

fn start_petstore_service() -> (may::coroutine::JoinHandle<()>, SocketAddr) {
    let (routes, _slug) = brrtrouter::load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let service = AppService { router, dispatcher };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    (handle, addr)
}

fn send_request(addr: &SocketAddr, req: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(req.as_bytes()).unwrap();
    stream.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
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

fn parse_response(resp: &str) -> (u16, Value) {
    let mut parts = resp.split("\r\n\r\n");
    let headers = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("");
    let status_line = headers.lines().next().unwrap_or("");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("0")
        .parse()
        .unwrap();
    let json: Value = serde_json::from_str(body).unwrap_or_default();
    (status, json)
}

#[test]
fn test_dispatch_success() {
    let (handle, addr) = start_petstore_service();
    let resp = send_request(&addr, "GET /pets HTTP/1.1\r\nHost: localhost\r\n\r\n");
    unsafe { handle.coroutine().cancel() };
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 200);
    assert!(body.get("items").is_some());
}

#[test]
fn test_route_404() {
    let (handle, addr) = start_petstore_service();
    let resp = send_request(&addr, "GET /nope HTTP/1.1\r\nHost: localhost\r\n\r\n");
    unsafe { handle.coroutine().cancel() };
    let (status, _body) = parse_response(&resp);
    assert_eq!(status, 404);
}

#[test]
fn test_panic_recovery() {
    fn panic_handler(_req: HandlerRequest) {
        panic!("boom");
    }
    let route = RouteMeta {
        method: Method::GET,
        path_pattern: "/panic".to_string(),
        handler_name: "panic".to_string(),
        parameters: Vec::new(),
        request_schema: None,
        response_schema: None,
        example: None,
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
    };
    let router = Router::new(vec![route]);
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("panic", panic_handler);
    }
    let service = AppService { router, dispatcher };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();

    let resp = send_request(&addr, "GET /panic HTTP/1.1\r\nHost: localhost\r\n\r\n");
    unsafe { handle.coroutine().cancel() };
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 500);
    assert!(body.get("error").is_some());
}

#[test]
fn test_headers_and_cookies() {
    fn header_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 200,
            body: json!({
                "headers": req.headers,
                "cookies": req.cookies,
            }),
        };
        let _ = req.reply_tx.send(response);
    }

    let route = RouteMeta {
        method: Method::GET,
        path_pattern: "/headertest".to_string(),
        handler_name: "header".to_string(),
        parameters: Vec::new(),
        request_schema: None,
        response_schema: None,
        example: None,
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
    };
    let router = Router::new(vec![route]);
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("header", header_handler);
    }
    let service = AppService { router, dispatcher };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();

    let request = concat!(
        "GET /headertest HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "X-Test: value\r\n",
        "X-Other: foo\r\n",
        "Cookie: session=abc123; theme=dark\r\n",
        "\r\n"
    );
    let resp = send_request(&addr, request);
    unsafe { handle.coroutine().cancel() };
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 200);
    assert_eq!(body["headers"]["x-test"], "value");
    assert_eq!(body["headers"]["x-other"], "foo");
    assert_eq!(body["cookies"]["session"], "abc123");
    assert_eq!(body["cookies"]["theme"], "dark");
}
