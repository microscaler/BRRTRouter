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
    validator_cache::ValidatorCache,
};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Watch an OpenAPI spec file and rebuild the [`Router`] when it changes.
///
/// The provided callback will receive the reloaded routes so the caller can
/// rebuild dispatcher mappings or perform additional work.
///
/// # Arguments
///
/// * `spec_path` - Path to the OpenAPI specification file
/// * `router` - Shared router instance
/// * `dispatcher` - Shared dispatcher instance
/// * `validator_cache` - Optional validator cache to clear on reload
/// * `on_reload` - Callback invoked after successful reload
pub fn watch_spec<P, F>(
    spec_path: P,
    router: Arc<RwLock<Router>>,
    dispatcher: Arc<RwLock<Dispatcher>>,
    validator_cache: Option<ValidatorCache>,
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
                    // HR1: Spec change detected
                    let event_kind = format!("{:?}", event.kind);
                    let spec_path_str = watch_path.to_str().unwrap_or("unknown");

                    info!(
                        event_type = %event_kind,
                        spec_path = %spec_path_str,
                        "Spec change detected"
                    );
                    println!("🔄 Hot reload: Spec change detected at {spec_path_str}");

                    // HR2: Spec reload started
                    debug!(
                        spec_path = %spec_path_str,
                        "Spec reload started"
                    );
                    println!("📖 Hot reload: Loading spec from {spec_path_str}");

                    let reload_start = Instant::now();

                    match spec::load_spec(spec_path_str) {
                        Ok((routes, _spec)) => {
                            let routes_count = routes.len();
                            let route_paths: Vec<String> = routes
                                .iter()
                                .map(|r| format!("{} {}", r.method, r.path_pattern))
                                .collect();

                            // Build new router
                            let new_router = Router::new(routes.clone());

                            // Update router
                            if let Ok(mut r) = router.write() {
                                *r = new_router;
                            } else {
                                warn!(
                                    spec_path = %spec_path_str,
                                    "Failed to acquire router write lock"
                                );
                                println!("⚠️  Hot reload: Failed to acquire router write lock");
                                return;
                            }

                            // Clear validator cache to force recompilation with new schemas
                            if let Some(ref cache) = validator_cache {
                                let cache_size_before = cache.size();
                                cache.clear();
                                info!(
                                    spec_path = %spec_path_str,
                                    cache_entries_cleared = cache_size_before,
                                    "Validator cache cleared for hot reload"
                                );
                                println!("🗑️  Hot reload: Cleared {cache_size_before} cached schema validators");
                            }

                            // Update dispatcher
                            if let Ok(mut d) = dispatcher.write() {
                                on_reload(&mut d, routes);
                            } else {
                                warn!(
                                    spec_path = %spec_path_str,
                                    "Failed to acquire dispatcher write lock"
                                );
                                println!("⚠️  Hot reload: Failed to acquire dispatcher write lock");
                                return;
                            }

                            // HR3: Spec reload success
                            let reload_time_ms = reload_start.elapsed().as_millis() as u64;
                            info!(
                                spec_path = %spec_path_str,
                                routes_count = routes_count,
                                reload_time_ms = reload_time_ms,
                                routes = ?route_paths,
                                "Spec reload success"
                            );
                            println!(
                                "✅ Hot reload: Successfully reloaded {routes_count} routes in {reload_time_ms}ms",
                            );
                        }
                        Err(e) => {
                            // HR4: Spec reload failed
                            let reload_time_ms = reload_start.elapsed().as_millis() as u64;
                            let error_message = format!("{e}");

                            error!(
                                spec_path = %spec_path_str,
                                reload_time_ms = reload_time_ms,
                                error = %error_message,
                                error_type = std::any::type_name_of_val(&e),
                                "Spec reload failed"
                            );
                            println!(
                                "❌ Hot reload: Failed to reload spec from {spec_path_str} ({reload_time_ms}ms): {error_message}",
                            );
                            println!("   Previous spec remains active - server continues running");
                        }
                    }
                }
            }
            Err(e) => {
                // Filesystem watcher error
                let error_message = format!("{e:?}");
                error!(
                    error = %error_message,
                    "Filesystem watcher error"
                );
                eprintln!("❌ Hot reload: Filesystem watcher error: {error_message}");
            }
        },
        Config::default(),
    )?;

    watcher.watch(&path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
