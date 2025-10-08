//! # Static Files Module
//!
//! The static files module provides secure static file serving with template rendering
//! support for BRRTRouter applications.
//!
//! ## Overview
//!
//! This module enables serving:
//! - Static HTML, CSS, JavaScript files
//! - Template files with dynamic content injection (using MiniJinja)
//! - API documentation pages
//! - Landing pages and SPAs
//!
//! ## Security
//!
//! The module includes path traversal protection:
//! - Blocks `../` and `..\\` sequences
//! - Validates path components
//! - Prevents access outside the base directory
//! - Rejects parent directory navigation attempts
//!
//! ## Template Rendering
//!
//! HTML files ending in `.html` are automatically processed as MiniJinja templates,
//! allowing dynamic content injection:
//!
//! ```html
//! <h1>{{ title }}</h1>
//! <p>{{ description }}</p>
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use brrtrouter::static_files::StaticFiles;
//! use serde_json::json;
//!
//! let static_files = StaticFiles::new("./static");
//!
//! // Serve a static file
//! let (content_type, body) = static_files.load("/index.html", None)
//!     .expect("Failed to load file");
//!
//! // Serve with template context
//! let ctx = json!({
//!     "title": "My API",
//!     "version": "1.0.0"
//! });
//! let (content_type, body) = static_files.load("/index.html", Some(&ctx))
//!     .expect("Failed to load file");
//! ```
//!
//! ## Supported MIME Types
//!
//! The module automatically detects content types:
//!
//! - `.html` → `text/html`
//! - `.css` → `text/css`
//! - `.js` → `application/javascript`
//! - `.json` → `application/json`
//! - `.txt` → `text/plain`
//! - Others → `application/octet-stream`
//!
//! ## Handler Integration
//!
//! ```rust,ignore
//! use brrtrouter::static_files::StaticFiles;
//! use brrtrouter::dispatcher::HandlerResponse;
//!
//! fn serve_docs(req: HandlerRequest) -> HandlerResponse {
//!     let static_files = StaticFiles::new("./docs");
//!     
//!     match static_files.load(req.path(), None) {
//!         Ok((content_type, body)) => {
//!             HandlerResponse::new(200)
//!                 .header("Content-Type", content_type)
//!                 .body(body)
//!         }
//!         Err(_) => HandlerResponse::new(404).body("Not Found"),
//!     }
//! }
//! ```

use minijinja::Environment;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Static file server with security and template rendering support.
///
/// Serves files from a base directory with path traversal protection
/// and automatic template rendering for HTML files.
#[derive(Clone)]
pub struct StaticFiles {
    base_dir: PathBuf,
}

impl StaticFiles {
    /// Create a new static file server for the given directory
    ///
    /// # Arguments
    ///
    /// * `base` - Base directory containing static files
    ///
    /// # Security
    ///
    /// Path traversal attacks are prevented - requests cannot escape the base directory.
    pub fn new<P: Into<PathBuf>>(base: P) -> Self {
        Self {
            base_dir: base.into(),
        }
    }

    fn map_path(&self, url_path: &str) -> Option<PathBuf> {
        let clean = url_path.trim_start_matches('/');
        if clean.contains("../")
            || clean.contains("/..")
            || clean.contains("..\\")
            || clean.contains("\\..")
        {
            return None;
        }
        let mut pb = self.base_dir.clone();
        for comp in Path::new(clean).components() {
            match comp {
                Component::Normal(s) => pb.push(s),
                Component::CurDir => {}
                Component::ParentDir => return None,
                _ => return None,
            }
        }
        Some(pb)
    }

    fn content_type(path: &Path) -> &'static str {
        match path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "html" => "text/html",
            "css" => "text/css",
            "js" => "application/javascript",
            "json" => "application/json",
            "txt" => "text/plain",
            _ => "application/octet-stream",
        }
    }

    /// Load a file from the static directory with optional template rendering
    ///
    /// If the file has an `.html` extension, it's rendered as a MiniJinja template
    /// with the provided context. Other files are returned as-is.
    ///
    /// # Arguments
    ///
    /// * `url_path` - URL path to the file (e.g., `/index.html`)
    /// * `ctx` - Optional JSON context for template rendering
    ///
    /// # Returns
    ///
    /// A tuple of `(file_contents, content_type)` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path is invalid or contains directory traversal attempts
    /// - The file doesn't exist
    /// - Template rendering fails (for HTML files)
    /// - File I/O fails
    pub fn load(
        &self,
        url_path: &str,
        ctx: Option<&JsonValue>,
    ) -> io::Result<(Vec<u8>, &'static str)> {
        let path = self
            .map_path(url_path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "invalid path"))?;
        if !path.exists() || !path.is_file() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        }
        if path.extension().and_then(|s| s.to_str()) == Some("html") {
            if let Some(ctx_val) = ctx {
                let source = fs::read_to_string(&path)?;
                let mut env = Environment::new();
                env.add_template("tpl", &source).map_err(io::Error::other)?;
                let tmpl = env.get_template("tpl").map_err(io::Error::other)?;
                let rendered = tmpl.render(ctx_val).map_err(io::Error::other)?;
                return Ok((rendered.into_bytes(), Self::content_type(&path)));
            }
        }
        let bytes = fs::read(&path)?;
        Ok((bytes, Self::content_type(&path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_map_path_prevents_traversal() {
        let sf = StaticFiles::new("tests/staticdata");
        assert!(sf.map_path("../Cargo.toml").is_none());
        let escaped_string = "..\\/..\\/Cargo.toml";
        assert!(sf.map_path(escaped_string).is_none());
    }

    #[test]
    fn test_load_plain_file() {
        let sf = StaticFiles::new("tests/staticdata");
        let (bytes, ct) = sf.load("hello.txt", None).unwrap();
        assert_eq!(ct, "text/plain");
        assert_eq!(String::from_utf8(bytes).unwrap(), "Hello\n");
    }

    #[test]
    fn test_render_html() {
        let sf = StaticFiles::new("tests/staticdata");
        let ctx = json!({ "name": "World" });
        let (bytes, ct) = sf.load("hello.html", Some(&ctx)).unwrap();
        assert_eq!(ct, "text/html");
        assert_eq!(String::from_utf8(bytes).unwrap(), "<h1>Hello World!</h1>");
    }

    #[test]
    fn test_load_js() {
        let sf = StaticFiles::new("tests/staticdata");
        let (bytes, ct) = sf.load("bundle.js", None).unwrap();
        assert_eq!(ct, "application/javascript");
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "console.log('bundled');\n"
        );
    }
}
