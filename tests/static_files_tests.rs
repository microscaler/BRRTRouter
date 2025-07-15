use brrtrouter::static_files::StaticFiles;
use serde_json::json;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

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
fn test_html_rendering_without_context() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.html");
    let mut file = fs::File::create(&file_path).unwrap();
    writeln!(file, "<h1>Static Content</h1>").unwrap();

    let sf = StaticFiles::new(dir.path());
    let (bytes, ct) = sf.load("test.html", None).unwrap();
    assert_eq!(ct, "text/html");
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "<h1>Static Content</h1>\n"
    );
}

#[test]
fn test_html_rendering_with_complex_context() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("complex.html");
    let mut file = fs::File::create(&file_path).unwrap();
    write!(
        file,
        "<h1>Hello {{{{ user.name }}}}!</h1><p>Age: {{{{ user.age }}}}</p>"
    )
    .unwrap();

    let sf = StaticFiles::new(dir.path());
    let ctx = json!({"user": {"name": "Alice", "age": 30}});
    let (bytes, ct) = sf.load("complex.html", Some(&ctx)).unwrap();
    assert_eq!(ct, "text/html");
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "<h1>Hello Alice!</h1><p>Age: 30</p>"
    );
}

#[test]
fn test_content_type_detection() {
    let dir = tempdir().unwrap();

    // Test various file extensions
    let test_cases = vec![
        ("test.css", "text/css", "body { color: red; }"),
        ("test.json", "application/json", r#"{"key": "value"}"#),
        ("test.txt", "text/plain", "Plain text content"),
        (
            "test.unknown",
            "application/octet-stream",
            "unknown content",
        ),
        ("no_extension", "application/octet-stream", "no extension"),
    ];

    for (filename, expected_ct, content) in test_cases {
        let file_path = dir.path().join(filename);
        let mut file = fs::File::create(&file_path).unwrap();
        write!(file, "{}", content).unwrap();

        let sf = StaticFiles::new(dir.path());
        let (bytes, ct) = sf.load(filename, None).unwrap();
        assert_eq!(ct, expected_ct, "Failed for file: {}", filename);
        assert_eq!(String::from_utf8(bytes).unwrap(), content);
    }
}

#[test]
fn test_directory_traversal_attacks() {
    let sf = StaticFiles::new("tests/staticdata");

    let malicious_paths = vec![
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "/etc/passwd",
        "\\windows\\system32\\config\\sam",
        "..%2F..%2F..%2Fetc%2Fpasswd",
        "..%5C..%5C..%5Cwindows%5Csystem32%5Cconfig%5Csam",
        "....//....//....//etc//passwd",
        "....\\\\....\\\\....\\\\windows\\\\system32\\\\config\\\\sam",
        "./.././.././../etc/passwd",
        ".\\..\\.\\..\\.\\..\\windows\\system32\\config\\sam",
    ];

    for path in malicious_paths {
        let result = sf.load(path, None);
        assert!(result.is_err(), "Should reject malicious path: {}", path);
    }
}

#[test]
fn test_nonexistent_file() {
    let sf = StaticFiles::new("tests/staticdata");
    let result = sf.load("nonexistent.txt", None);
    assert!(result.is_err());
}

#[test]
fn test_nonexistent_directory() {
    let sf = StaticFiles::new("nonexistent_directory");
    let result = sf.load("any_file.txt", None);
    assert!(result.is_err());
}

#[test]
fn test_empty_path() {
    let sf = StaticFiles::new("tests/staticdata");
    let result = sf.load("", None);
    assert!(result.is_err());
}

#[test]
fn test_root_path() {
    let sf = StaticFiles::new("tests/staticdata");
    let result = sf.load("/", None);
    assert!(result.is_err());
}

#[test]
fn test_path_with_leading_slash() {
    let sf = StaticFiles::new("tests/staticdata");
    let (bytes, ct) = sf.load("/hello.txt", None).unwrap();
    assert_eq!(ct, "text/plain");
    assert_eq!(String::from_utf8(bytes).unwrap(), "Hello\n");
}

#[test]
fn test_case_sensitive_extensions() {
    let dir = tempdir().unwrap();

    let test_cases = vec![
        ("test.HTML", "text/html"),
        ("test.CSS", "text/css"),
        ("test.JS", "application/javascript"),
        ("test.JSON", "application/json"),
        ("test.TXT", "text/plain"),
    ];

    for (filename, expected_ct) in test_cases {
        let file_path = dir.path().join(filename);
        let mut file = fs::File::create(&file_path).unwrap();
        write!(file, "test content").unwrap();

        let sf = StaticFiles::new(dir.path());
        let (_, ct) = sf.load(filename, None).unwrap();
        assert_eq!(
            ct, expected_ct,
            "Failed for uppercase extension: {}",
            filename
        );
    }
}

#[test]
fn test_template_rendering_error() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("invalid.html");
    let mut file = fs::File::create(&file_path).unwrap();
    writeln!(file, "<h1>Hello {{{{ invalid_syntax}}</h1>").unwrap();

    let sf = StaticFiles::new(dir.path());
    let ctx = json!({"name": "Test"});
    let result = sf.load("invalid.html", Some(&ctx));
    assert!(result.is_err());
}

#[test]
fn test_large_file_handling() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("large.txt");
    let large_content = "x".repeat(1024 * 1024); // 1MB file
    let mut file = fs::File::create(&file_path).unwrap();
    write!(file, "{}", large_content).unwrap();

    let sf = StaticFiles::new(dir.path());
    let (bytes, ct) = sf.load("large.txt", None).unwrap();
    assert_eq!(ct, "text/plain");
    assert_eq!(bytes.len(), 1024 * 1024);
    assert_eq!(String::from_utf8(bytes).unwrap(), large_content);
}

#[test]
fn test_nested_directory_access() {
    let dir = tempdir().unwrap();
    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    let file_path = subdir.join("nested.txt");
    let mut file = fs::File::create(&file_path).unwrap();
    write!(file, "nested content").unwrap();

    let sf = StaticFiles::new(dir.path());
    let (bytes, ct) = sf.load("subdir/nested.txt", None).unwrap();
    assert_eq!(ct, "text/plain");
    assert_eq!(String::from_utf8(bytes).unwrap(), "nested content");
}

#[test]
fn test_staticfiles_clone() {
    let sf1 = StaticFiles::new("tests/staticdata");
    let sf2 = sf1.clone();

    // Both should work independently
    let (bytes1, ct1) = sf1.load("hello.txt", None).unwrap();
    let (bytes2, ct2) = sf2.load("hello.txt", None).unwrap();

    assert_eq!(ct1, ct2);
    assert_eq!(bytes1, bytes2);
    assert_eq!(String::from_utf8(bytes1).unwrap(), "Hello\n");
}

#[test]
fn test_current_directory_components() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let mut file = fs::File::create(&file_path).unwrap();
    write!(file, "test content").unwrap();

    let sf = StaticFiles::new(dir.path());

    // These should all work (current directory components should be ignored)
    let paths = vec!["./test.txt", "././test.txt", "./././test.txt"];

    for path in paths {
        let (bytes, ct) = sf.load(path, None).unwrap();
        assert_eq!(ct, "text/plain");
        assert_eq!(String::from_utf8(bytes).unwrap(), "test content");
    }
}
