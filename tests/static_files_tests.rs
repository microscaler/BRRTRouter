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
fn test_traversal_prevented() {
    let sf = StaticFiles::new("tests/staticdata");
    assert!(sf.load("../Cargo.toml", None).is_err());
}
