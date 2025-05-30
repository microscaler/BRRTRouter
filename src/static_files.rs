use minijinja::Environment;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub struct StaticFiles {
    base_dir: PathBuf,
}

impl StaticFiles {
    pub fn new<P: Into<PathBuf>>(base: P) -> Self {
        Self { base_dir: base.into() }
    }

    fn map_path(&self, url_path: &str) -> Option<PathBuf> {
        let mut pb = self.base_dir.clone();
        for comp in Path::new(url_path.trim_start_matches('/')).components() {
            match comp {
                Component::Normal(s) => pb.push(s),
                Component::CurDir => {}
                _ => return None,
            }
        }
        Some(pb)
    }

    fn content_type(path: &Path) -> &'static str {
        match path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase().as_str() {
            "html" => "text/html",
            "css" => "text/css",
            "js" => "application/javascript",
            "json" => "application/json",
            "txt" => "text/plain",
            _ => "application/octet-stream",
        }
    }

    pub fn load(&self, url_path: &str, ctx: Option<&JsonValue>) -> io::Result<(Vec<u8>, &'static str)> {
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
                env.add_template("tpl", &source).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                let tmpl = env.get_template("tpl").map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                let rendered = tmpl
                    .render(ctx_val)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
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
        assert!(sf.map_path("..\/..\/Cargo.toml").is_none());
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
}
