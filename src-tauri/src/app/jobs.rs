use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub id: String,
    pub kind: String,
    pub state: JobState,
    pub progress: f32,
    pub message: Option<String>,
    pub created_at: u64,
}

struct JobEntry {
    info: JobInfo,
    cancel_flag: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct JobRegistry {
    jobs: Mutex<HashMap<String, JobEntry>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
        }
    }

    pub fn create_job(&self, kind: impl Into<String>) -> (String, Arc<AtomicBool>) {
        let kind_str = kind.into();
        let id = format!("{}_{}", kind_str, uuid_v4_simple());
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let info = JobInfo {
            id: id.clone(),
            kind: kind_str,
            state: JobState::Running,
            progress: 0.0,
            message: None,
            created_at,
        };

        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        lock.insert(
            id.clone(),
            JobEntry {
                info,
                cancel_flag: cancel_flag.clone(),
            },
        );

        (id, cancel_flag)
    }

    pub fn update_progress(&self, id: &str, progress: f32, message: Option<String>) {
        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = lock.get_mut(id) {
            entry.info.progress = progress.clamp(0.0, 100.0);
            if message.is_some() {
                entry.info.message = message;
            }
        }
    }

    pub fn complete_job(&self, id: &str) {
        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = lock.get_mut(id) {
            entry.info.state = JobState::Succeeded;
            entry.info.progress = 100.0;
        }
    }

    pub fn fail_job(&self, id: &str, reason: &str) {
        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = lock.get_mut(id) {
            entry.info.state = JobState::Failed;
            entry.info.message = Some(reason.to_string());
        }
    }

    pub fn cancel_job(&self, id: &str) -> bool {
        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = lock.get_mut(id) {
            entry.cancel_flag.store(true, Ordering::SeqCst);
            entry.info.state = JobState::Cancelled;
            true
        } else {
            false
        }
    }

    pub fn is_cancelled(&self, id: &str) -> bool {
        let lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        lock.get(id)
            .map(|e| e.cancel_flag.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    pub fn get_job(&self, id: &str) -> Option<JobInfo> {
        let lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        lock.get(id).map(|e| e.info.clone())
    }
}

fn uuid_v4_simple() -> String {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let rand_part = (nanos ^ 0x5deece66d) as u64;
    format!("{:016x}{:016x}", nanos as u64, rand_part)
}
