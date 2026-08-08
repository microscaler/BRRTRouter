//! Epic 13.4 — streaming uploads + HttpFile download helpers (fixtures).

use brrtrouter::server::{
    parse_multipart_form_data_streaming, sanitize_filename, MultipartStreamOptions, TempUpload,
    MULTIPART_FILE_TOO_LARGE, MULTIPART_MALFORMED, MULTIPART_MISSING_BOUNDARY,
};
use brrtrouter::typed::{HandlerResponseOutput, HttpFile};
use std::fs;

fn sample_multipart(boundary: &str, body: &str) -> Vec<u8> {
    format!("--{boundary}\r\n{body}--{boundary}--\r\n").into_bytes()
}

fn opts() -> MultipartStreamOptions {
    // Unique per call — parallel tests must not share/remove the same directory.
    let dir = std::env::temp_dir().join(format!(
        "brrtrouter-epic13-4-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    MultipartStreamOptions {
        max_file_part_bytes: 1024,
        temp_dir: dir,
    }
}

#[test]
fn epic13_4_p1_stream_to_temp() {
    let o = opts();
    let raw = sample_multipart(
        "b",
        concat!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"note.bin\"\r\n",
            "Content-Type: application/octet-stream\r\n\r\n",
            "hello\r\n",
        ),
    );
    let v =
        parse_multipart_form_data_streaming(&raw, "multipart/form-data; boundary=b", &o).unwrap();
    assert_eq!(v["file"]["encoding"], "file");
    assert_eq!(v["file"]["size"], 5);
    let path = v["file"]["path"].as_str().unwrap();
    assert_eq!(fs::read(path).unwrap(), b"hello");
    let _ = fs::remove_file(path);
    let _ = fs::remove_dir_all(&o.temp_dir);
}

#[test]
fn epic13_4_p2_mixed_text_and_file() {
    let o = opts();
    let raw = sample_multipart(
        "b",
        concat!(
            "Content-Disposition: form-data; name=\"title\"\r\n\r\n",
            "Ada\r\n",
            "--b\r\n",
            "Content-Disposition: form-data; name=\"file\"; filename=\"a.txt\"\r\n",
            "Content-Type: text/plain\r\n\r\n",
            "x\r\n",
        ),
    );
    let v =
        parse_multipart_form_data_streaming(&raw, "multipart/form-data; boundary=b", &o).unwrap();
    assert_eq!(v["title"], "Ada");
    assert_eq!(v["file"]["encoding"], "file");
    let path = v["file"]["path"].as_str().unwrap();
    let _ = fs::remove_file(path);
    let _ = fs::remove_dir_all(&o.temp_dir);
}

#[test]
fn epic13_4_p3_download_attachment() {
    let hr = HttpFile::attachment("r.pdf", "application/pdf", b"%PDF".to_vec())
        .into_handler_response()
        .unwrap();
    let disp = hr.get_header("content-disposition").unwrap();
    assert!(disp.starts_with("attachment"));
    assert!(disp.contains("r.pdf"));
}

#[test]
fn epic13_4_p6_download_content_type() {
    let hr = HttpFile::inline("text/plain", b"hi".to_vec())
        .into_handler_response()
        .unwrap();
    assert_eq!(hr.get_header("content-type"), Some("text/plain"));
}

#[test]
fn epic13_4_n1_over_cap() {
    let mut o = opts();
    o.max_file_part_bytes = 3;
    let raw = sample_multipart(
        "b",
        concat!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"big.bin\"\r\n",
            "Content-Type: application/octet-stream\r\n\r\n",
            "12345\r\n",
        ),
    );
    let err = parse_multipart_form_data_streaming(&raw, "multipart/form-data; boundary=b", &o)
        .unwrap_err();
    assert_eq!(err, MULTIPART_FILE_TOO_LARGE);
    let _ = fs::remove_dir_all(&o.temp_dir);
}

#[test]
fn epic13_4_n2_malformed() {
    let o = opts();
    let err = parse_multipart_form_data_streaming(b"nope", "multipart/form-data; boundary=abc", &o)
        .unwrap_err();
    assert_eq!(err, MULTIPART_MALFORMED);
    let _ = fs::remove_dir_all(&o.temp_dir);
}

#[test]
fn epic13_4_n3_missing_boundary() {
    let o = opts();
    let err = parse_multipart_form_data_streaming(b"x", "multipart/form-data", &o).unwrap_err();
    assert_eq!(err, MULTIPART_MISSING_BOUNDARY);
}

#[test]
fn epic13_4_n6_path_traversal_sanitized() {
    assert_eq!(sanitize_filename("../../../etc/passwd"), "passwd");
}

#[test]
fn epic13_4_docs_updated() {
    let md = include_str!("../docs/multipart.md");
    assert!(md.contains("13.4") || md.contains("Streaming"));
    assert!(md.contains("HttpFile"));
    assert!(md.contains("encoding: \"file\""));
}

#[test]
fn epic13_4_temp_upload_persist() {
    let dir = opts().temp_dir;
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("keep.bin");
    fs::write(&path, b"z").unwrap();
    let kept = TempUpload::new(path.clone()).persist();
    assert!(kept.exists());
    let _ = fs::remove_file(&kept);
    let _ = fs::remove_dir_all(&dir);
}
