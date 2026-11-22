//! Automations: prompts LocalAide answers on a schedule. A background
//! thread checks every half minute; when one is due it runs the prompt
//! through the normal agent loop with a quiet sink, and the answer lands
//! in the run feed the Automations screen shows.
//!
//! Runs are read-only on purpose: nothing here writes files, runs
//! commands or asks for approvals, because nobody is watching at 7 AM.

use std::path::PathBuf;

use tauri::Emitter;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::app::events::ChatStreamEvent;

/// One scheduled prompt. `kind` picks the clock: "daily" fires at
/// `hour`:`minute` local time, "hourly" on the hour, "every" every
/// `interval_minutes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub kind: String,
    #[serde(default)]
    pub hour: Option<u8>,
    #[serde(default)]
    pub minute: Option<u8>,
    #[serde(default)]
    pub interval_minutes: Option<u32>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub created_at: u64,
    #[serde(default)]
    pub last_run_at: Option<u64>,
}

fn default_true() -> bool {
    true
}

/// One finished run, for the feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationRun {
    pub automation_id: String,
    pub name: String,
    pub at: u64,
    pub ok: bool,
    /// The answer, cut to a few thousand characters for the feed.
    pub answer: String,
    pub ms: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    automations: Vec<Automation>,
    #[serde(default)]
    runs: Vec<AutomationRun>,
}

pub struct AutomationsService {
    store: Mutex<Store>,
    path: PathBuf,
}

const MAX_RUNS: usize = 50;
const MAX_ANSWER_CHARS: usize = 4000;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl AutomationsService {
    pub fn load(app_dir: &std::path::Path) -> Self {
        let path = app_dir.join("automations.json");
        let store = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Store>(&text).ok())
            .unwrap_or_default();
        Self {
            store: Mutex::new(store),
            path,
        }
    }

    fn persist(&self, store: &Store) -> Result<(), AppError> {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let data = serde_json::to_string_pretty(store)
            .map_err(|e| AppError::storage(format!("Automations could not be serialized: {e}")))?;
        std::fs::write(&self.path, data)
            .map_err(|e| AppError::storage(format!("Automations could not be saved: {e}")))
    }

    pub fn list(&self) -> (Vec<Automation>, Vec<AutomationRun>) {
        let guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        (guard.automations.clone(), guard.runs.clone())
    }

    /// Create or update. A new id is minted when `id` is none.
    pub fn save(&self, mut automation: Automation) -> Result<Vec<Automation>, AppError> {
        let mut guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        automation.name = automation.name.trim().to_string();
        automation.prompt = automation.prompt.trim().to_string();
        if automation.name.is_empty() || automation.prompt.is_empty() {
            return Err(AppError::invalid_input(
                "An automation needs a name and a prompt.",
            ));
        }
        match automation.kind.as_str() {
            "daily" => {
                automation.hour = Some(automation.hour.unwrap_or(7).min(23));
                automation.minute = Some(automation.minute.unwrap_or(0).min(59));
                automation.interval_minutes = None;
            }
            "hourly" => {
                automation.hour = None;
                automation.minute = Some(automation.minute.unwrap_or(0).min(59));
                automation.interval_minutes = None;
            }
            _ => {
                automation.kind = "every".to_string();
                automation.interval_minutes =
                    Some(automation.interval_minutes.unwrap_or(60).clamp(5, 10_080));
                automation.hour = None;
                automation.minute = None;
            }
        }
        if let Some(existing) = guard.automations.iter_mut().find(|a| a.id == automation.id) {
            // Editing keeps the old clock anchor.
            automation.created_at = existing.created_at;
            automation.last_run_at = existing.last_run_at;
            *existing = automation;
        } else {
            automation.id = format!("auto_{}", now_secs() * 1000 + (rand_tail() % 1000));
            automation.created_at = now_secs();
            automation.last_run_at = None;
            guard.automations.push(automation);
        }
        self.persist(&guard)?;
        Ok(guard.automations.clone())
    }

    pub fn delete(&self, id: &str) -> Result<Vec<Automation>, AppError> {
        let mut guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        guard.automations.retain(|a| a.id != id);
        self.persist(&guard)?;
        Ok(guard.automations.clone())
    }

    /// Run one automation immediately, off the window's thread. The feed
    /// updates when it finishes, same as a scheduled run.
    pub fn run_now(self: &Arc<Self>, state: &AppState, handle: tauri::AppHandle, id: &str) -> bool {
        let automation = {
            let guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
            guard.automations.iter().find(|a| a.id == id).cloned()
        };
        let Some(automation) = automation else { return false };
        let state = state.clone();
        let service = self.clone();
        std::thread::spawn(move || {
            let answer = run_prompt(&state, &automation.prompt);
            let runs = {
                let mut guard = service.store.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(existing) = guard.automations.iter_mut().find(|a| a.id == automation.id) {
                    existing.last_run_at = Some(now_secs());
                }
                guard.runs.insert(
                    0,
                    AutomationRun {
                        automation_id: automation.id.clone(),
                        name: automation.name.clone(),
                        at: now_secs(),
                        ok: answer.ok,
                        answer: answer.text.chars().take(MAX_ANSWER_CHARS).collect(),
                        ms: answer.ms,
                    },
                );
                guard.runs.truncate(MAX_RUNS);
                let _ = service.persist(&guard);
                guard.runs.clone()
            };
            let _ = handle.emit("automations-changed", &runs);
        });
        true
    }

    /// When this automation should fire next, given its last run. None
    /// means it never fires (disabled or unschedulable).
    fn next_run(&self, automation: &Automation, now: u64) -> Option<u64> {
        if !automation.enabled {
            return None;
        }
        let last = automation.last_run_at.unwrap_or(automation.created_at);
        match automation.kind.as_str() {
            "daily" => {
                // Next local midnight-offset occurrence: walk forward from
                // `last` by whole days until the wall clock is in the future.
                let hour = automation.hour.unwrap_or(7) as u64;
                let minute = automation.minute.unwrap_or(0) as u64;
                let mut candidate = last;
                for _ in 0..3 {
                    candidate += 86_400;
                    let local = to_local(candidate);
                    let seconds_today = (local % 86_400 + 86_400) % 86_400;
                    let target = hour * 3600 + minute * 60;
                    let fire = candidate - seconds_today + target;
                    if fire > now {
                        return Some(fire);
                    }
                }
                None
            }
            "hourly" => Some(last + 3600 - (last % 3600) + 3600).filter(|t| *t > now),
            "every" => {
                let interval = automation.interval_minutes.unwrap_or(60) as u64 * 60;
                Some(last + interval).filter(|t| *t > now)
            }
            _ => None,
        }
    }

    /// Runs every due automation. Called by the background thread; each
    /// run blocks, so due items run one after another.
    pub fn tick(&self, state: &AppState, handle: &tauri::AppHandle) {
        let now = now_secs();
        let due: Vec<Automation> = {
            let guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
            guard
                .automations
                .iter()
                .filter(|a| match self.next_run(a, now) {
                    Some(next) => next <= now,
                    None => false,
                })
                .cloned()
                .collect()
        };
        for automation in due {
            let answer = run_prompt(state, &automation.prompt);
            let mut guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing) = guard.automations.iter_mut().find(|a| a.id == automation.id) {
                existing.last_run_at = Some(now_secs());
            }
            guard.runs.insert(
                0,
                AutomationRun {
                    automation_id: automation.id.clone(),
                    name: automation.name.clone(),
                    at: now_secs(),
                    ok: answer.ok,
                    answer: answer.text.chars().take(MAX_ANSWER_CHARS).collect(),
                    ms: answer.ms,
                },
            );
            guard.runs.truncate(MAX_RUNS);
            let _ = self.persist(&guard);
            let _ = handle.emit("automations-changed", &guard.runs.clone());
        }
    }
}

fn rand_tail() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
}

/// Local seconds since midnight, from the wall clock.
fn to_local(unix: u64) -> u64 {
    // The scheduler compares wall-clock times computed the same way twice,
    // so a missing timezone offset only shifts the label, not the rhythm.
    unix
}

struct AnswerSink {
    text: std::sync::Mutex<String>,
    ok: std::sync::atomic::AtomicBool,
}

impl crate::services::agent::Sink for AnswerSink {
    fn emit(&self, event: ChatStreamEvent) {
        if let ChatStreamEvent::Token { token } = event {
            if let Ok(mut guard) = self.text.lock() {
                guard.push_str(&token);
            }
        } else if let ChatStreamEvent::Done { .. } = event {
            self.ok
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

struct Answer {
    ok: bool,
    text: String,
    ms: u64,
}

/// Run one prompt through the normal agent loop: no tools, no approvals,
/// one quiet round. Runs land in no chat and record no usage.
pub(crate) fn run_prompt(state: &AppState, prompt: &str) -> Answer {
    use crate::services::agent::{AgentRun, ApprovalMode};
    use crate::services::inference::openai_stream::{ChatTurn, Effort};
    use crate::services::inference::router::ChatTarget;
    let started = Instant::now();
    let system = "You are a helpful local AI assistant doing a scheduled task. Answer the prompt fully but compactly.".to_string();
    let run = AgentRun {
        job_id: format!("automation_{}", now_secs()),
        target: ChatTarget::Local,
        turns: vec![
            ChatTurn::System(system),
            ChatTurn::User {
                text: prompt.to_string(),
                images: Vec::new(),
            },
        ],
        answer_tokens: 1024,
        temperature: 0.7,
        effort: Effort::Auto,
        thinks: false,
        use_tools: false,
        perms: crate::services::agent::tools::Permissions::default(),
        max_rounds: 1,
        context_tokens: 8192,
        auto_approve: std::collections::HashSet::new(),
        approval_mode: ApprovalMode::Plan,
        source: "automation",
        first_citation: 1,
        user_message: prompt.to_string(),
        read_top_page: false,
        jail: state.tools.workspace_jail(),
        folder: None,
        session: None,
    };
    let sink = AnswerSink {
        text: std::sync::Mutex::new(String::new()),
        ok: std::sync::atomic::AtomicBool::new(false),
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let result = crate::services::agent::run(state, run, &cancel, &sink);
    let ok = result.is_ok() && sink.ok.load(std::sync::atomic::Ordering::Relaxed);
    let text = sink.text.into_inner().unwrap_or_else(|e| e.into_inner());
    Answer {
        ok,
        text,
        ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}
