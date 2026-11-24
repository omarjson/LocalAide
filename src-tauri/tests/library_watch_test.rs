//! Debounce coverage for the library index watcher (backlog item 6.3).
//! The updater is a recorder, so no SQLite lock or disk database is involved.

use app_lib::app::error::AppError;
use app_lib::services::library::watch::{LibraryUpdater, LibraryWatcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Default)]
struct Recorder {
    calls: Arc<Mutex<Vec<PathBuf>>>,
    removals: Arc<Mutex<Vec<String>>>,
}

impl LibraryUpdater for Recorder {
    fn update_file(&self, path: &Path) -> Result<(), AppError> {
        self.calls.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    fn remove_file(&self, path: &str) -> Result<bool, AppError> {
        self.removals.lock().unwrap().push(path.to_string());
        Ok(true)
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("localaide-watch-{name}-{stamp}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn wait_for<F: Fn() -> bool>(what: &str, predicate: F) {
    let deadline = SystemTime::now() + Duration::from_secs(6);
    while SystemTime::now() < deadline {
        if predicate() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

#[test]
fn rapid_edits_are_debounced_and_unsupported_files_are_ignored() {
    let dir = temp_dir("debounce");
    let note = dir.join("note.md");
    let ignored = dir.join("image.png");
    let recorder = Recorder::default();
    let watcher = LibraryWatcher::new();
    watcher
        .start_watch(&dir, Arc::new(recorder.clone()))
        .unwrap();
    thread::sleep(Duration::from_millis(200));

    for i in 0..5 {
        fs::write(&note, format!("revision {i}")).unwrap();
        thread::sleep(Duration::from_millis(20));
    }
    fs::write(&ignored, b"not text").unwrap();

    wait_for("the debounced update", || {
        !recorder.calls.lock().unwrap().is_empty()
    });
    // Let a second debounce window pass: a late duplicate would show up here.
    thread::sleep(Duration::from_millis(900));
    watcher.stop_watch();

    let calls = recorder.calls.lock().unwrap();
    let mut counts: HashMap<_, usize> = HashMap::new();
    for path in calls.iter() {
        *counts.entry(path.clone()).or_default() += 1;
    }
    assert_eq!(
        counts.get(&note),
        Some(&1),
        "five writes should become one update: {counts:?}"
    );
    assert!(
        !counts.contains_key(&ignored),
        "unsupported extensions must be skipped"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_remove_event_is_forwarded_to_the_updater() {
    let dir = temp_dir("remove");
    let note = dir.join("gone.md");
    fs::write(&note, "temporary").unwrap();
    let recorder = Recorder::default();
    let watcher = LibraryWatcher::new();
    watcher
        .start_watch(&dir, Arc::new(recorder.clone()))
        .unwrap();
    thread::sleep(Duration::from_millis(200));
    fs::remove_file(&note).unwrap();
    wait_for("the removal event", || {
        !recorder.removals.lock().unwrap().is_empty()
    });
    watcher.stop_watch();
    let removals = recorder.removals.lock().unwrap();
    assert!(
        removals.iter().any(|s| s.ends_with("gone.md")),
        "{removals:?}"
    );
    drop(removals);
    let _ = fs::remove_dir_all(&dir);
}
