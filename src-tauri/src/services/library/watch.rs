use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use crate::app::error::AppError;
use crate::services::library::ingest::FileIngester;

pub trait LibraryUpdater: Send + Sync + 'static {
    fn update_file(&self, path: &Path) -> Result<(), AppError>;
    fn remove_file(&self, path: &str) -> Result<bool, AppError>;
}

pub struct LibraryWatcher {
    watcher: Mutex<Option<RecommendedWatcher>>,
}

impl LibraryWatcher {
    pub fn new() -> Self {
        Self {
            watcher: Mutex::new(None),
        }
    }

    pub fn start_watch<U: LibraryUpdater>(&self, folder: &Path, updater: Arc<U>) -> Result<(), AppError> {
        if !folder.exists() || !folder.is_dir() {
            return Err(AppError::invalid_input(format!("Directory does not exist: {}", folder.display())));
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        ).map_err(|e| AppError::internal(format!("Failed to initialize file watcher: {e}")))?;

        watcher.watch(folder, RecursiveMode::Recursive)
            .map_err(|e| AppError::storage(format!("Failed to watch directory {}: {e}", folder.display())))?;

        // Background debouncer thread
        std::thread::spawn(move || {
            let mut pending_files: HashSet<PathBuf> = HashSet::new();
            let mut removed_files: HashSet<PathBuf> = HashSet::new();

            loop {
                let start_batch = std::time::Instant::now();
                // 500ms debounce batching window
                while start_batch.elapsed() < Duration::from_millis(500) {
                    match rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(event) => match event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any => {
                                for p in event.paths {
                                    if !FileIngester::should_ignore_path(&p)
                                        && FileIngester::is_supported_extension(&p)
                                        && p.is_file()
                                    {
                                        pending_files.insert(p);
                                    }
                                }
                            }
                            EventKind::Remove(_) => {
                                for p in event.paths {
                                    removed_files.insert(p);
                                }
                            }
                            _ => {}
                        },
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }

                // Process pending updates
                for p in pending_files.drain() {
                    if let Err(e) = updater.update_file(&p) {
                        log::debug!("File watcher update skipped for {}: {e}", p.display());
                    }
                }

                // Process pending removals
                for p in removed_files.drain() {
                    let s = p.to_string_lossy().to_string();
                    let _ = updater.remove_file(&s);
                }
            }
        });

        let mut lock = self.watcher.lock()
            .map_err(|_| AppError::internal("Watcher lock poisoned"))?;
        *lock = Some(watcher);

        Ok(())
    }
}
