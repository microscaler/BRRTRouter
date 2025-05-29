use crate::{
    router::Router,
    spec::{self, RouteMeta},
};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Watch an OpenAPI spec file and rebuild the [`Router`] when it changes.
///
/// The provided callback will receive the reloaded routes so the caller can
/// rebuild dispatcher mappings or perform additional work.
pub fn watch_spec<P, F>(
    spec_path: P,
    router: Arc<RwLock<Router>>,
    mut on_reload: F,
) -> notify::Result<RecommendedWatcher>
where
    P: AsRef<Path>,
    F: FnMut(Vec<RouteMeta>) + Send + 'static,
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
                        on_reload(routes);
                    }
                }
            }
            Err(e) => eprintln!("watch error: {:?}", e),
        },
        Config::default(),
    )?;

    watcher.watch(&path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
