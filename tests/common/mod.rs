pub mod temp_files {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Global counter and lock for thread-safe temporary file creation
    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
    static TEMP_LOCK: Mutex<()> = Mutex::new(());

    /// Creates a temporary file with guaranteed unique name to prevent race conditions
    pub fn create_temp_spec(content: &str, ext: &str) -> PathBuf {
        let _lock = TEMP_LOCK.lock().unwrap();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "brrt_test_{}_{}_{}.{}",
            std::process::id(),
            counter,
            nanos,
            ext
        ));

        std::fs::write(&path, content).unwrap();
        path
    }

    /// Creates a temporary file with default yaml extension
    pub fn create_temp_yaml(content: &str) -> PathBuf {
        create_temp_spec(content, "yaml")
    }

    /// Creates a temporary file with json extension
    pub fn create_temp_json(content: &str) -> PathBuf {
        create_temp_spec(content, "json")
    }

    /// Cleanup temporary files (best effort)
    pub fn cleanup_temp_files(paths: &[PathBuf]) {
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub mod test_server {
    use std::sync::Once;

    /// Ensures May coroutines are configured only once
    static MAY_INIT: Once = Once::new();

    pub fn setup_may_runtime() {
        MAY_INIT.call_once(|| {
            may::config().set_stack_size(0x8000);
        });
    }
}
