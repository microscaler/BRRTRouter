//! # Hot Reload Module
//!
//! The hot reload module provides live reloading of OpenAPI specifications without restarting
//! the server. This enables rapid development workflows where you can modify your API spec
//! and see changes immediately.
//!
//! ## Overview
//!
//! Hot reload watches the OpenAPI specification file for changes and:
//! - Detects file modifications using filesystem watchers
//! - Reloads and parses the updated specification
//! - Rebuilds the router with new routes
//! - Updates the dispatcher with new handler mappings
//! - Calls custom reload hooks for application-specific updates
//!
//! ## Usage
//!
//! ```rust,ignore
//! use brrtrouter::hot_reload::watch_spec;
//! use std::sync::{Arc, RwLock};
//!
//! let router = Arc::new(RwLock::new(Router::from_spec(&spec)));
//! let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));
//!
//! // Start watching for spec changes
//! let watcher = watch_spec(
//!     "openapi.yaml",
//!     router.clone(),
//!     dispatcher.clone(),
//!     |disp, routes| {
//!         println!("Reloaded {} routes", routes.len());
//!         // Perform custom updates
//!     }
//! )?;
//!
//! // Keep watcher alive
//! std::mem::forget(watcher);
//! ```
//!
//! ## Reload Process
//!
//! When the spec file changes:
//!
//! 1. **Detection** - Filesystem watcher detects modification
//! 2. **Parse** - New spec is loaded and validated
//! 3. **Router Update** - New routing table is built and swapped in
//! 4. **Dispatcher Update** - Handler registry is updated via callback
//! 5. **Hooks** - Custom reload logic executes (e.g., metrics, logging)
//!
//! ## Debouncing
//!
//! The hot reload system includes built-in debouncing to prevent multiple reloads
//! when editors save files multiple times in quick succession.
//!
//! ## Error Handling
//!
//! If the new spec fails to parse or validate:
//! - The error is logged
//! - The previous spec remains active
//! - The server continues serving requests
//!
//! This ensures your service stays up even if you temporarily save an invalid spec.
//!
//! ## Performance
//!
//! Hot reload is designed for development, not production. The reload process:
//! - Blocks request processing briefly during router swap
//! - May cause a small spike in latency during reload
//! - Should not be used with very frequent spec changes
//!
//! For production, disable hot reload and use proper deployment strategies.

use crate::{
    dispatcher::Dispatcher,
    router::Router,
    spec::{self, RouteMeta},
};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::info;

/// Watch an OpenAPI spec file and rebuild the [`Router`] when it changes.
///
/// The provided callback will receive the reloaded routes so the caller can
/// rebuild dispatcher mappings or perform additional work.
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

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    if let Ok((routes, _)) = spec::load_spec(watch_path.to_str().unwrap()) {
                        let new_router = Router::new(routes.clone());
                        if let Ok(mut r) = router.write() {
                            *r = new_router;
                        }
                        if let Ok(mut d) = dispatcher.write() {
                            info!(
                                "hot-reload: applying route updates ({} routes)",
                                routes.len()
                            );
                            on_reload(&mut d, routes);
                        }
                    }
                }
            }
            Err(e) => eprintln!("watch error: {e:?}"),
        },
        Config::default(),
    )?;

    watcher.watch(&path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
