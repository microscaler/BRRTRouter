use brrtrouter::static_files::StaticFiles;
use serde_json::json;

#[test]
fn test_html_rendering() {
    let sf = StaticFiles::new("tests/staticdata");
    let ctx = json!({"name": "Integration"});
    let (bytes, ct) = sf.load("hello.html", Some(&ctx)).unwrap();
    assert_eq!(ct, "text/html");
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "<h1>Hello Integration!</h1>"
    );
}

#[test]
fn test_js_bundle() {
    let sf = StaticFiles::new("tests/staticdata");
    let (bytes, ct) = sf.load("bundle.js", None).unwrap();
    assert_eq!(ct, "application/javascript");
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "console.log('bundled');\n"
    );
}

#[test]
fn test_traversal_prevented() {
    let sf = StaticFiles::new("tests/staticdata");
    assert!(sf.load("../Cargo.toml", None).is_err());
    assert!(sf.load("..\\Cargo.toml", None).is_err());
}

#[test]
fn test_addition_template_renders_sum() {
    let sf = StaticFiles::new("tests/staticdata");
    let a = 7;
    let b = 13;
    let ctx = json!({
        "a": a,
        "b": b,
        "sum": a + b,
    });
    let (bytes, ct) = sf.load("add.html", Some(&ctx)).unwrap();
    assert_eq!(ct, "text/html");
    let body = String::from_utf8(bytes).unwrap();
    assert!(body.contains(&format!("{} + {} = {}", a, b, a + b)));
}
