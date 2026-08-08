//! Streaming multipart file parts to disk (Epic 13.4).
//!
//! Text fields remain JSON (Story 12.6). File parts are written to a secure temp
//! directory and exposed as `{ encoding: "file", path, filename, content_type, size }`.
//!
//! On parse failure, any temp files created during the attempt are removed (NFR-2).
//! Handler ownership: after a successful parse, the caller owns the paths and should
//! delete them (or wrap with [`TempUpload`]) when finished.

use serde_json::{json, Map, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::multipart::{
    extract_boundary, MULTIPART_FILE_TOO_LARGE, MULTIPART_MALFORMED, MULTIPART_MISSING_BOUNDARY,
};

/// Default max octets for a **streamed** file part (64 MiB).
pub const DEFAULT_MAX_STREAMED_FILE_PART_BYTES: usize = 64 * 1024 * 1024;

/// Env: when `1`/`true`/`yes`, [`crate::server::parse_request`] streams file parts to disk.
pub const MULTIPART_STREAM_ENV: &str = "BRRTR_MULTIPART_STREAM_FILES";

/// Subdirectory name under the process temp dir.
pub const TEMP_UPLOAD_SUBDIR: &str = "brrtrouter-uploads";

/// RAII temp upload path — deletes the file on drop unless [`TempUpload::persist`] was called.
#[derive(Debug)]
pub struct TempUpload {
    path: PathBuf,
    persist: bool,
}

impl TempUpload {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            persist: false,
        }
    }

    /// Keep the file after drop (handler takes permanent ownership).
    pub fn persist(mut self) -> PathBuf {
        self.persist = true;
        self.path.clone()
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempUpload {
    fn drop(&mut self) {
        if !self.persist {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Options for [`parse_multipart_form_data_streaming`].
#[derive(Debug, Clone)]
pub struct MultipartStreamOptions {
    /// Max octets per file part (413 when exceeded).
    pub max_file_part_bytes: usize,
    /// Directory for temp files (created if missing).
    pub temp_dir: PathBuf,
}

impl Default for MultipartStreamOptions {
    fn default() -> Self {
        Self {
            max_file_part_bytes: DEFAULT_MAX_STREAMED_FILE_PART_BYTES,
            temp_dir: default_upload_temp_dir(),
        }
    }
}

/// Default temp dir: `$TMPDIR/brrtrouter-uploads` (or OS temp).
#[must_use]
pub fn default_upload_temp_dir() -> PathBuf {
    std::env::temp_dir().join(TEMP_UPLOAD_SUBDIR)
}

/// Whether request parsing should stream file parts (env opt-in).
#[must_use]
pub fn multipart_stream_files_enabled() -> bool {
    match std::env::var(MULTIPART_STREAM_ENV) {
        Ok(v) => {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

/// Parse multipart, streaming file parts to disk under `opts.temp_dir`.
///
/// Text fields → JSON scalars (same as MVP-A). File parts → JSON objects with
/// `encoding: "file"` and `path` (absolute). Does not hold file bytes in a `String`.
///
/// # Errors
///
/// Same markers as [`super::multipart::parse_multipart_form_data`], plus sink I/O
/// mapped to [`MULTIPART_MALFORMED`] (caller may map to 500 if desired — we keep
/// stable markers; disk errors use malformed for fail-closed without panic).
pub fn parse_multipart_form_data_streaming(
    raw: &[u8],
    content_type: &str,
    opts: &MultipartStreamOptions,
) -> Result<Value, String> {
    let Some(boundary) = extract_boundary(content_type) else {
        return Err(MULTIPART_MISSING_BOUNDARY.to_string());
    };
    ensure_temp_dir(&opts.temp_dir)?;
    let mut created: Vec<PathBuf> = Vec::new();
    match parse_streaming_inner(raw, &boundary, opts, &mut created) {
        Ok(v) => Ok(v),
        Err(e) => {
            for p in &created {
                let _ = fs::remove_file(p);
            }
            Err(e)
        }
    }
}

fn ensure_temp_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|_| MULTIPART_MALFORMED.to_string())?;
    // Best-effort restrictive perms on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

fn parse_streaming_inner(
    raw: &[u8],
    boundary: &str,
    opts: &MultipartStreamOptions,
    created: &mut Vec<PathBuf>,
) -> Result<Value, String> {
    let delim = format!("--{boundary}");
    let delim_bytes = delim.as_bytes();
    let close = format!("--{boundary}--");
    let close_bytes = close.as_bytes();

    let mut parts: Vec<&[u8]> = Vec::new();
    let mut rest = raw;
    loop {
        if let Some(idx) = find_subslice(rest, delim_bytes) {
            let after = &rest[idx + delim_bytes.len()..];
            if after.starts_with(b"--") {
                break;
            }
            let after = strip_leading_crlf(after);
            if let Some(next) = find_subslice(after, delim_bytes) {
                let mut chunk = &after[..next];
                chunk = strip_trailing_crlf(chunk);
                if !chunk.is_empty() {
                    parts.push(chunk);
                }
                rest = &after[next..];
            } else if let Some(next) = find_subslice(after, close_bytes) {
                let mut chunk = &after[..next];
                chunk = strip_trailing_crlf(chunk);
                if !chunk.is_empty() {
                    parts.push(chunk);
                }
                break;
            } else {
                let chunk = strip_trailing_crlf(after);
                if !chunk.is_empty() {
                    parts.push(chunk);
                }
                break;
            }
        } else if rest.is_empty() && parts.is_empty() {
            return Err(MULTIPART_MALFORMED.to_string());
        } else {
            break;
        }
    }

    if parts.is_empty() && !raw.is_empty() {
        return Err(MULTIPART_MALFORMED.to_string());
    }

    let mut map = Map::new();
    for part in parts {
        let (headers, body) = split_part_headers_body(part)?;
        let (name, filename, part_ct) = parse_part_disposition(&headers)?;
        if name.is_empty() {
            return Err(MULTIPART_MALFORMED.to_string());
        }
        if let Some(fname) = filename {
            let safe_name = sanitize_filename(&fname);
            if body.len() > opts.max_file_part_bytes {
                return Err(MULTIPART_FILE_TOO_LARGE.to_string());
            }
            let path = write_part_to_temp(&opts.temp_dir, &safe_name, body, created)?;
            let ct = part_ct.unwrap_or_else(|| "application/octet-stream".into());
            let mut file_obj = Map::new();
            file_obj.insert("filename".into(), Value::String(fname));
            file_obj.insert("content_type".into(), Value::String(ct));
            file_obj.insert("size".into(), json!(body.len()));
            file_obj.insert("encoding".into(), Value::String("file".into()));
            file_obj.insert(
                "path".into(),
                Value::String(path.to_string_lossy().into_owned()),
            );
            map.insert(name, Value::Object(file_obj));
        } else {
            let text = std::str::from_utf8(body)
                .map_err(|_| MULTIPART_MALFORMED.to_string())?
                .to_string();
            map.insert(name, loose_scalar(&text));
        }
    }

    Ok(Value::Object(map))
}

fn write_part_to_temp(
    dir: &Path,
    safe_name: &str,
    body: &[u8],
    created: &mut Vec<PathBuf>,
) -> Result<PathBuf, String> {
    let unique = format!("up-{}-{}-{}", std::process::id(), created.len(), safe_name);
    // N6: reject path traversal in the generated name.
    if unique.contains("..") || unique.contains('/') || unique.contains('\\') {
        return Err(MULTIPART_MALFORMED.to_string());
    }
    let path = dir.join(&unique);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| MULTIPART_MALFORMED.to_string())?;
    file.write_all(body)
        .map_err(|_| MULTIPART_MALFORMED.to_string())?;
    file.flush().map_err(|_| MULTIPART_MALFORMED.to_string())?;
    created.push(path.clone());
    Ok(path)
}

/// Strip path components; allow only a short basename of safe chars.
#[must_use]
pub fn sanitize_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("upload.bin");
    let mut out = String::new();
    for c in base.chars().take(128) {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() || out == "." || out == ".." {
        "upload.bin".into()
    } else {
        out
    }
}

fn loose_scalar(s: &str) -> Value {
    if s == "true" {
        return Value::Bool(true);
    }
    if s == "false" {
        return Value::Bool(false);
    }
    if let Ok(n) = s.parse::<i64>() {
        return json!(n);
    }
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return Value::Number(num);
        }
    }
    Value::String(s.to_string())
}

fn split_part_headers_body(part: &[u8]) -> Result<(String, &[u8]), String> {
    let sep = find_subslice(part, b"\r\n\r\n")
        .or_else(|| find_subslice(part, b"\n\n"))
        .ok_or_else(|| MULTIPART_MALFORMED.to_string())?;
    let (hdr, rest) = if part[sep..].starts_with(b"\r\n\r\n") {
        (&part[..sep], &part[sep + 4..])
    } else {
        (&part[..sep], &part[sep + 2..])
    };
    let headers = std::str::from_utf8(hdr).map_err(|_| MULTIPART_MALFORMED.to_string())?;
    Ok((headers.to_string(), rest))
}

fn parse_part_disposition(
    headers: &str,
) -> Result<(String, Option<String>, Option<String>), String> {
    let mut name = None;
    let mut filename = None;
    let mut content_type = None;
    for line in headers.lines() {
        let line = line.trim();
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("content-disposition:") {
            for token in line.split(';').skip(1) {
                let token = token.trim();
                let Some((k, v)) = token.split_once('=') else {
                    continue;
                };
                if k.trim().eq_ignore_ascii_case("name") {
                    name = Some(unquote(v));
                } else if k.trim().eq_ignore_ascii_case("filename") {
                    filename = Some(unquote(v));
                }
            }
        } else if lower.starts_with("content-type:") {
            content_type = Some(
                line.split_once(':')
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default(),
            );
        }
    }
    let name = name.ok_or_else(|| MULTIPART_MALFORMED.to_string())?;
    Ok((name, filename, content_type))
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').to_string()
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn strip_leading_crlf(b: &[u8]) -> &[u8] {
    if b.starts_with(b"\r\n") {
        &b[2..]
    } else if b.starts_with(b"\n") {
        &b[1..]
    } else {
        b
    }
}

fn strip_trailing_crlf(b: &[u8]) -> &[u8] {
    if b.ends_with(b"\r\n") {
        &b[..b.len() - 2]
    } else if b.ends_with(b"\n") {
        &b[..b.len() - 1]
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_multipart(boundary: &str, body: &str) -> Vec<u8> {
        format!("--{boundary}\r\n{body}--{boundary}--\r\n").into_bytes()
    }

    fn temp_opts() -> MultipartStreamOptions {
        let dir = std::env::temp_dir().join(format!(
            "brrtrouter-test-uploads-{}-{}",
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
    fn p1_small_file_streams_to_temp() {
        let opts = temp_opts();
        let boundary = "b";
        let inner = concat!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"note.bin\"\r\n",
            "Content-Type: application/octet-stream\r\n\r\n",
            "hello\r\n",
        );
        let raw = sample_multipart(boundary, inner);
        let v = parse_multipart_form_data_streaming(&raw, "multipart/form-data; boundary=b", &opts)
            .unwrap();
        assert_eq!(v["file"]["encoding"], "file");
        assert_eq!(v["file"]["size"], 5);
        assert_eq!(v["file"]["filename"], "note.bin");
        let path = v["file"]["path"].as_str().unwrap();
        let bytes = fs::read(path).unwrap();
        assert_eq!(bytes, b"hello");
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(&opts.temp_dir);
    }

    #[test]
    fn p2_text_and_file_mixed() {
        let opts = temp_opts();
        let boundary = "b";
        let inner = concat!(
            "Content-Disposition: form-data; name=\"title\"\r\n\r\n",
            "Ada\r\n",
            "--b\r\n",
            "Content-Disposition: form-data; name=\"file\"; filename=\"a.txt\"\r\n",
            "Content-Type: text/plain\r\n\r\n",
            "x\r\n",
        );
        let raw = sample_multipart(boundary, inner);
        let v = parse_multipart_form_data_streaming(&raw, "multipart/form-data; boundary=b", &opts)
            .unwrap();
        assert_eq!(v["title"], "Ada");
        assert_eq!(v["file"]["encoding"], "file");
        let path = v["file"]["path"].as_str().unwrap();
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(&opts.temp_dir);
    }

    #[test]
    fn p5_utf8_text_field_unchanged() {
        let opts = temp_opts();
        let boundary = "b";
        let inner = "Content-Disposition: form-data; name=\"name\"\r\n\r\nAda\r\n";
        let raw = sample_multipart(boundary, inner);
        let v = parse_multipart_form_data_streaming(&raw, "multipart/form-data; boundary=b", &opts)
            .unwrap();
        assert_eq!(v["name"], "Ada");
        let _ = fs::remove_dir_all(&opts.temp_dir);
    }

    #[test]
    fn n1_file_over_cap_413_marker() {
        let mut opts = temp_opts();
        opts.max_file_part_bytes = 4;
        let boundary = "b";
        let inner = concat!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"big.bin\"\r\n",
            "Content-Type: application/octet-stream\r\n\r\n",
            "12345\r\n",
        );
        let raw = sample_multipart(boundary, inner);
        let err =
            parse_multipart_form_data_streaming(&raw, "multipart/form-data; boundary=b", &opts)
                .unwrap_err();
        assert_eq!(err, MULTIPART_FILE_TOO_LARGE);
        // No leftover files
        assert!(
            !opts.temp_dir.exists()
                || fs::read_dir(&opts.temp_dir).map(|d| d.count()).unwrap_or(0) == 0
        );
        let _ = fs::remove_dir_all(&opts.temp_dir);
    }

    #[test]
    fn n2_malformed_no_panic() {
        let opts = temp_opts();
        let err = parse_multipart_form_data_streaming(
            b"not-multipart",
            "multipart/form-data; boundary=abc",
            &opts,
        )
        .unwrap_err();
        assert_eq!(err, MULTIPART_MALFORMED);
        let _ = fs::remove_dir_all(&opts.temp_dir);
    }

    #[test]
    fn n3_missing_boundary() {
        let opts = temp_opts();
        let err =
            parse_multipart_form_data_streaming(b"x", "multipart/form-data", &opts).unwrap_err();
        assert_eq!(err, MULTIPART_MISSING_BOUNDARY);
    }

    #[test]
    fn n6_sanitize_rejects_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename(".."), "upload.bin");
    }

    #[test]
    fn n7_empty_file_part_ok() {
        let opts = temp_opts();
        let boundary = "b";
        let inner = concat!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"empty.bin\"\r\n",
            "Content-Type: application/octet-stream\r\n\r\n",
            "\r\n",
        );
        let raw = sample_multipart(boundary, inner);
        let v = parse_multipart_form_data_streaming(&raw, "multipart/form-data; boundary=b", &opts)
            .unwrap();
        assert_eq!(v["file"]["size"], 0);
        let path = v["file"]["path"].as_str().unwrap();
        assert!(fs::read(path).unwrap().is_empty());
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(&opts.temp_dir);
    }

    #[test]
    fn temp_upload_cleans_on_drop() {
        let dir = temp_opts().temp_dir;
        ensure_temp_dir(&dir).unwrap();
        let path = dir.join("t.bin");
        fs::write(&path, b"x").unwrap();
        {
            let t = TempUpload::new(path.clone());
            assert!(t.path().exists());
        }
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
