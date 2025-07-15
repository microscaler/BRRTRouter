use crate::{
    dispatcher::Dispatcher,
    router::Router,
    spec::{self, RouteMeta},
};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant};

/// Hot reload state to prevent infinite loops
struct HotReloadState {
    last_reload: Instant,
    debounce_duration: Duration,
    is_reloading: bool,
}

impl HotReloadState {
    fn new() -> Self {
        Self {
            last_reload: Instant::now(),
            debounce_duration: Duration::from_millis(500), // 500ms debounce
            is_reloading: false,
        }
    }

    fn should_reload(&mut self) -> bool {
        let now = Instant::now();
        if self.is_reloading {
            return false; // Already reloading, skip
        }
        if now.duration_since(self.last_reload) < self.debounce_duration {
            return false; // Too soon, debounce
        }
        self.is_reloading = true;
        self.last_reload = now;
        true
    }

    fn reload_complete(&mut self) {
        self.is_reloading = false;
    }
}

/// Watch an OpenAPI spec file and rebuild the [`Router`] when it changes.
///
/// The provided callback will receive the reloaded routes so the caller can
/// rebuild dispatcher mappings or perform additional work.
/// 
/// This implementation includes debouncing to prevent infinite reload loops
/// that can occur when file changes trigger more file changes.
pub fn watch_spec<P, F>(
    spec_path: P,
    router: Arc<RwLock<Router>>,
    dispatcher: Arc<RwLock<Dispatcher>>,
    mut on_reload: F,
) -> notify::Result<RecommendedWatcher>
where
    P: AsRef<Path>,
    F: FnMut(&mut Dispatcher, Vec<RouteMeta>) + Send + 'static,
{
    let path: PathBuf = spec_path.as_ref().to_path_buf();
    let watch_path = path.clone();
    let reload_state = Arc::new(Mutex::new(HotReloadState::new()));

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    // Check if we should reload (debouncing)
                    let should_reload = {
                        if let Ok(mut state) = reload_state.lock() {
                            state.should_reload()
                        } else {
                            false
                        }
                    };

                    if !should_reload {
                        return; // Skip this reload event
                    }

                    match spec::load_spec(watch_path.to_str().unwrap()) {
                        Ok((routes, _)) => {
                            // Update router
                            if let Ok(mut r) = router.write() {
                                let new_router = Router::new(routes.clone());
                                *r = new_router;
                            }
                            
                            // Update dispatcher via callback
                            if let Ok(mut d) = dispatcher.write() {
                                on_reload(&mut d, routes);
                            }
                        }
                        Err(e) => {
                            eprintln!("🔄 Hot reload: Failed to load spec: {e:?}");
                        }
                    }

                    // Mark reload as complete
                    if let Ok(mut state) = reload_state.lock() {
                        state.reload_complete();
                    }
                }
            }
            Err(e) => eprintln!("🔄 Hot reload: Watch error: {e:?}"),
        },
        Config::default(),
    )?;

    watcher.watch(&path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
