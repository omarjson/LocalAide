use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Paused,
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
    /// When the job reached a final state, in seconds since 1970.
    finished_at: Option<u64>,
}

/// Finished jobs stay this long, so a late status check still finds them.
const FINISHED_JOB_TTL_SECS: u64 = 600;

impl JobState {
    /// Succeeded, failed and cancelled are final. A final state never changes.
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            JobState::Succeeded | JobState::Failed | JobState::Cancelled | JobState::Paused
        )
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Drops jobs that finished more than `FINISHED_JOB_TTL_SECS` before `now`.
fn prune_finished(jobs: &mut HashMap<String, JobEntry>, now: u64) {
    jobs.retain(|_, entry| {
        entry
            .finished_at
            .is_none_or(|at| now.saturating_sub(at) < FINISHED_JOB_TTL_SECS)
    });
}

/// Moves a job to a final state, unless it already has one. A cancel that
/// arrives first is kept, so a late "succeeded" cannot hide it.
fn finish(entry: &mut JobEntry, state: JobState, now: u64) -> bool {
    if entry.info.state.is_final() {
        return false;
    }
    entry.info.state = state;
    entry.finished_at = Some(now);
    true
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
        let created_at = now_secs();

        let info = JobInfo {
            id: id.clone(),
            kind: kind_str,
            state: JobState::Running,
            progress: 0.0,
            message: None,
            created_at,
        };

        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        // Every chat answer and download makes a job, so old ones are dropped here.
        prune_finished(&mut lock, created_at);
        lock.insert(
            id.clone(),
            JobEntry {
                info,
                cancel_flag: cancel_flag.clone(),
                finished_at: None,
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
        if let Some(entry) = lock.get_mut(id)
            && finish(entry, JobState::Succeeded, now_secs())
        {
            entry.info.progress = 100.0;
        }
    }

    pub fn fail_job(&self, id: &str, reason: &str) {
        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = lock.get_mut(id)
            && finish(entry, JobState::Failed, now_secs())
        {
            entry.info.message = Some(reason.to_string());
        }
    }

    pub fn cancel_job(&self, id: &str) -> bool {
        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = lock.get_mut(id) {
            entry.cancel_flag.store(true, Ordering::SeqCst);
            finish(entry, JobState::Cancelled, now_secs());
            true
        } else {
            false
        }
    }

    /// Cancels every unfinished job made with this `kind`. Job ids add a
    /// random part to the kind, so a caller that knows only the kind (such
    /// as "download_<model id>") cancels through this. Returns how many.
    pub fn cancel_kind(&self, kind: &str) -> usize {
        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        let now = now_secs();
        let mut cancelled = 0;
        for entry in lock.values_mut() {
            if entry.info.kind == kind && !entry.info.state.is_final() {
                entry.cancel_flag.store(true, Ordering::SeqCst);
                finish(entry, JobState::Cancelled, now);
                cancelled += 1;
            }
        }
        cancelled
    }

    /// Pauses every unfinished job made with this kind. The active request is
    /// cancelled, but the `.part` file remains and a later download resumes
    /// it with HTTP Range.
    pub fn pause_kind(&self, kind: &str) -> usize {
        let mut lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        let now = now_secs();
        let mut paused = 0;
        for entry in lock.values_mut() {
            if entry.info.kind == kind && !entry.info.state.is_final() {
                entry.cancel_flag.store(true, Ordering::SeqCst);
                if finish(entry, JobState::Paused, now) {
                    paused += 1;
                }
            }
        }
        paused
    }

    pub fn is_paused(&self, id: &str) -> bool {
        let lock = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        lock.get(id)
            .is_some_and(|e| e.info.state == JobState::Paused)
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

/// A suffix that makes job ids unique: the time, so ids differ between runs,
/// plus a counter, since two jobs can start within one clock tick.
fn uuid_v4_simple() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let count = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    format!("{nanos:016x}{count:08x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_final_state_is_never_replaced() {
        let registry = JobRegistry::new();
        let (id, flag) = registry.create_job("download_x");
        assert!(registry.cancel_job(&id));
        registry.complete_job(&id);
        registry.fail_job(&id, "late error");
        let job = registry.get_job(&id).unwrap();
        assert_eq!(job.state, JobState::Cancelled);
        assert!(job.message.is_none());
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn cancel_kind_reaches_jobs_by_their_kind() {
        let registry = JobRegistry::new();
        let (first, first_flag) = registry.create_job("preset_model-a");
        let (_, other_flag) = registry.create_job("preset_model-b");
        assert_eq!(registry.cancel_kind("preset_model-a"), 1);
        assert!(first_flag.load(Ordering::SeqCst));
        assert!(!other_flag.load(Ordering::SeqCst));
        assert_eq!(registry.get_job(&first).unwrap().state, JobState::Cancelled);
        // A finished job is not counted again.
        assert_eq!(registry.cancel_kind("preset_model-a"), 0);
    }

    #[test]
    fn finished_jobs_expire_and_running_jobs_stay() {
        let registry = JobRegistry::new();
        let (done, _) = registry.create_job("chat");
        let (running, _) = registry.create_job("chat");
        registry.complete_job(&done);

        let mut jobs = registry.jobs.lock().unwrap();
        let finished = jobs[&done].finished_at.unwrap();
        prune_finished(&mut jobs, finished + FINISHED_JOB_TTL_SECS - 1);
        assert!(jobs.contains_key(&done));
        prune_finished(&mut jobs, finished + FINISHED_JOB_TTL_SECS);
        assert!(!jobs.contains_key(&done));
        assert!(jobs.contains_key(&running));
    }

    #[test]
    fn job_ids_made_back_to_back_differ() {
        let registry = JobRegistry::new();
        let ids: std::collections::HashSet<String> =
            (0..1000).map(|_| registry.create_job("chat").0).collect();
        assert_eq!(ids.len(), 1000);
    }

    #[test]
    fn pause_kind_preserves_the_partial_download_state() {
        let registry = JobRegistry::new();
        let (id, flag) = registry.create_job("download_model-a");
        assert_eq!(registry.pause_kind("download_model-a"), 1);
        assert!(flag.load(Ordering::SeqCst));
        assert_eq!(registry.get_job(&id).unwrap().state, JobState::Paused);
        assert!(registry.is_paused(&id));
        registry.complete_job(&id);
        assert_eq!(registry.get_job(&id).unwrap().state, JobState::Paused);
    }
}
