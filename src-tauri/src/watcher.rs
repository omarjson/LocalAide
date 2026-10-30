use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::indexer::Indexer;

pub struct IndexWatcher {
    watcher: Mutex<Option<RecommendedWatcher>>,
    indexer: Arc<Indexer>,
}

impl IndexWatcher {
    pub fn new(indexer: Arc<Indexer>) -> Self {
        Self {
            watcher: Mutex::new(None),
            indexer,
        }
    }

    pub fn start_watch(&self, folder: &str) -> Result<(), String> {
        let folder_path = PathBuf::from(folder);
        if !folder_path.exists() || !folder_path.is_dir() {
            return Err(format!("folder not found: {}", folder));
        }

        let indexer = self.indexer.clone();
        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        )
        .map_err(|e| e.to_string())?;

        watcher
            .watch(&folder_path, RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;

        // Debounced batch processor
        let indexer_clone = indexer.clone();
        std::thread::spawn(move || {
            let mut pending: HashSet<PathBuf> = HashSet::new();
            let mut to_remove: HashSet<PathBuf> = HashSet::new();
            loop {
                // Drain the channel for a short window to batch events.
                let start = std::time::Instant::now();
                while start.elapsed() < Duration::from_millis(500) {
                    match rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(event) => {
                            match event.kind {
                                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any => {
                                    for path in event.paths {
                                        if path.is_file() {
                                            pending.insert(path);
                                        }
                                    }
                                }
                                EventKind::Remove(_) => {
                                    for path in event.paths {
                                        if path.is_file() {
                                            to_remove.insert(path);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        Err(_) => break,
                    }
                }

                if !pending.is_empty() {
                    let paths: Vec<String> = pending
                        .drain()
                        .filter_map(|p| p.to_str().map(|s| s.to_string()))
                        .collect();
                    if !paths.is_empty() {
                        // Batch commit to avoid one commit per file.
                        let _ = indexer_clone.index_files(&paths);
                    }
                }

                if !to_remove.is_empty() {
                    for path in to_remove.drain() {
                        if let Some(s) = path.to_str() {
                            let _ = indexer_clone.remove(s);
                        }
                    }
                }
            }
        });

        let mut lock = self.watcher.lock().unwrap();
        *lock = Some(watcher);
        Ok(())
    }
}

