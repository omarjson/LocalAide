//! "Check my setup" (Settings → General): download the Tiny chat model if
//! it is missing, load it, and get one reply. Each step reports pass or
//! fail with its time, so the whole path can be checked in one click.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::commands::chat::load_and_activate;
use crate::model_manager::Preset;
use crate::services::inference::openai_stream::{
    ChatTurn, Delta, Effort, initial_options, stream_with_retries,
};
use crate::services::inference::router::ChatTarget;

/// One step's state, sent while the check runs and kept in the report.
#[derive(Debug, Clone, Serialize)]
pub struct SelfTestStep {
    /// "model", "load" or "reply".
    pub step: &'static str,
    pub label: &'static str,
    /// "running", "pass", "fail" or "skip".
    pub status: &'static str,
    pub ms: u128,
    pub detail: String,
    /// Download progress, 0 to 100, while the model downloads.
    pub percent: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelfTestReport {
    pub passed: bool,
    pub model_id: String,
    pub steps: Vec<SelfTestStep>,
    pub ms: u128,
}

/// The chat model the Tiny preset installs.
fn tiny_chat_model(state: &AppState) -> Option<String> {
    Preset::tiny()
        .models
        .into_iter()
        .find(|id| state.model_service.is_chat_model(id))
}

/// Run the check. `emit` gets every step update; the share page and the
/// app both use this.
pub async fn run_selftest(
    state: &AppState,
    emit: Arc<dyn Fn(SelfTestStep) + Send + Sync>,
) -> SelfTestReport {
    let started = Instant::now();
    let mut steps: Vec<SelfTestStep> = Vec::new();
    let mut push = |step: SelfTestStep, emit: &Arc<dyn Fn(SelfTestStep) + Send + Sync>| {
        emit(step.clone());
        steps.push(step);
    };
    let skip = |step: &'static str, label: &'static str| SelfTestStep {
        step,
        label,
        status: "skip",
        ms: 0,
        detail: "Skipped because an earlier step failed.".into(),
        percent: None,
    };

    let Some(model_id) = tiny_chat_model(state) else {
        push(
            SelfTestStep {
                step: "model",
                label: "Tiny model on this computer",
                status: "fail",
                ms: 0,
                detail: "The model catalog has no Tiny chat model.".into(),
                percent: None,
            },
            &emit,
        );
        return SelfTestReport {
            passed: false,
            model_id: String::new(),
            steps,
            ms: started.elapsed().as_millis(),
        };
    };
    let name = state
        .model_service
        .get_model_entry(&model_id)
        .map(|e| e.name)
        .unwrap_or_else(|| model_id.clone());

    // 1. The model file.
    let t = Instant::now();
    let model_ok = if state.model_service.is_installed(&model_id) {
        push(
            SelfTestStep {
                step: "model",
                label: "Tiny model on this computer",
                status: "pass",
                ms: t.elapsed().as_millis(),
                detail: format!(
                    "{name} is installed. Its checksum was checked when it was downloaded."
                ),
                percent: None,
            },
            &emit,
        );
        true
    } else {
        emit(SelfTestStep {
            step: "model",
            label: "Tiny model on this computer",
            status: "running",
            ms: 0,
            detail: format!("Downloading {name}…"),
            percent: Some(0.0),
        });
        let (job_id, cancel) = state.jobs.create_job(format!("download_{model_id}"));
        let progress_emit = emit.clone();
        let progress_name = name.clone();
        let res = state
            .model_service
            .download_model(&model_id, cancel, move |p| {
                progress_emit(SelfTestStep {
                    step: "model",
                    label: "Tiny model on this computer",
                    status: "running",
                    ms: 0,
                    detail: format!("Downloading {progress_name}…"),
                    percent: Some(p.percent),
                });
            })
            .await;
        match res {
            Ok(_) => {
                state.jobs.complete_job(&job_id);
                push(
                    SelfTestStep {
                        step: "model",
                        label: "Tiny model on this computer",
                        status: "pass",
                        ms: t.elapsed().as_millis(),
                        detail: format!("Downloaded {name}; the checksum matches."),
                        percent: Some(100.0),
                    },
                    &emit,
                );
                true
            }
            Err(e) => {
                state.jobs.fail_job(&job_id, &e.message);
                push(
                    SelfTestStep {
                        step: "model",
                        label: "Tiny model on this computer",
                        status: "fail",
                        ms: t.elapsed().as_millis(),
                        detail: format!("The download failed: {}", e.message),
                        percent: None,
                    },
                    &emit,
                );
                false
            }
        }
    };
    if !model_ok {
        push(skip("load", "Load the model"), &emit);
        push(skip("reply", "First reply"), &emit);
        return SelfTestReport {
            passed: false,
            model_id,
            steps,
            ms: started.elapsed().as_millis(),
        };
    }

    // 2. Load it.
    let t = Instant::now();
    emit(SelfTestStep {
        step: "load",
        label: "Load the model",
        status: "running",
        ms: 0,
        detail: format!("Starting {name}…"),
        percent: None,
    });
    let already = state.model_manager.active_model_id().as_deref() == Some(model_id.as_str())
        && state.inference_engine.local().endpoint().is_some();
    let load = if already {
        Ok(String::new())
    } else {
        load_and_activate(state, model_id.clone()).await
    };
    let loaded = match load {
        Ok(_) => {
            push(
                SelfTestStep {
                    step: "load",
                    label: "Load the model",
                    status: "pass",
                    ms: t.elapsed().as_millis(),
                    detail: if already {
                        format!("{name} was already running.")
                    } else {
                        format!("{name} is running on this computer.")
                    },
                    percent: None,
                },
                &emit,
            );
            true
        }
        Err(e) => {
            push(
                SelfTestStep {
                    step: "load",
                    label: "Load the model",
                    status: "fail",
                    ms: t.elapsed().as_millis(),
                    detail: format!("The model did not start: {}", e.message),
                    percent: None,
                },
                &emit,
            );
            false
        }
    };
    if !loaded {
        push(skip("reply", "First reply"), &emit);
        return SelfTestReport {
            passed: false,
            model_id,
            steps,
            ms: started.elapsed().as_millis(),
        };
    }

    // 3. One reply from the local model, never an online one.
    let t = Instant::now();
    emit(SelfTestStep {
        step: "reply",
        label: "First reply",
        status: "running",
        ms: 0,
        detail: "Asking the model to say hello…".into(),
        percent: None,
    });
    let engine = state.inference_engine.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result = (|| {
            let endpoint = engine.endpoint_for(&ChatTarget::Local)?;
            let mut turns = vec![
                ChatTurn::System("Answer in one short sentence.".into()),
                ChatTurn::User {
                    text: "Say hello to the user in five words or fewer.".into(),
                    images: Vec::new(),
                },
            ];
            let mut opts = initial_options(endpoint.flavor, 256, 0.2, Vec::new(), Effort::Off);
            let cancel = AtomicBool::new(false);
            let deadline = Instant::now() + Duration::from_secs(120);
            let mut text = String::new();
            let (outcome, _) = stream_with_retries(
                engine.chat_client(),
                &endpoint,
                &mut turns,
                &mut opts,
                &cancel,
                &mut |d| {
                    if let Delta::Content(t) = d {
                        text.push_str(&t);
                    }
                    if Instant::now() > deadline {
                        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                },
            )?;
            Ok::<_, AppError>((text, outcome.reasoning, outcome.cancelled))
        })();
        let _ = tx.send(result);
    });
    let reply = rx
        .await
        .unwrap_or_else(|_| Err(AppError::internal("The reply check stopped unexpectedly.")));
    let passed = match reply {
        Ok((text, reasoning, cancelled))
            if !cancelled && (!text.trim().is_empty() || !reasoning.trim().is_empty()) =>
        {
            let shown = if text.trim().is_empty() {
                reasoning
            } else {
                text
            };
            push(
                SelfTestStep {
                    step: "reply",
                    label: "First reply",
                    status: "pass",
                    ms: t.elapsed().as_millis(),
                    detail: format!("“{}”", shown.trim().chars().take(160).collect::<String>()),
                    percent: None,
                },
                &emit,
            );
            true
        }
        Ok((_, _, true)) => {
            push(
                SelfTestStep {
                    step: "reply",
                    label: "First reply",
                    status: "fail",
                    ms: t.elapsed().as_millis(),
                    detail: "No reply within 2 minutes.".into(),
                    percent: None,
                },
                &emit,
            );
            false
        }
        Ok(_) => {
            push(
                SelfTestStep {
                    step: "reply",
                    label: "First reply",
                    status: "fail",
                    ms: t.elapsed().as_millis(),
                    detail: "The model answered with nothing.".into(),
                    percent: None,
                },
                &emit,
            );
            false
        }
        Err(e) => {
            push(
                SelfTestStep {
                    step: "reply",
                    label: "First reply",
                    status: "fail",
                    ms: t.elapsed().as_millis(),
                    detail: e.message,
                    percent: None,
                },
                &emit,
            );
            false
        }
    };
    SelfTestReport {
        passed,
        model_id,
        steps,
        ms: started.elapsed().as_millis(),
    }
}

#[tauri::command]
pub async fn selftest_run(
    state: State<'_, AppState>,
    channel: tauri::ipc::Channel<SelfTestStep>,
) -> Result<SelfTestReport, AppError> {
    let emit: Arc<dyn Fn(SelfTestStep) + Send + Sync> = Arc::new(move |s| {
        let _ = channel.send(s);
    });
    Ok(run_selftest(&state, emit).await)
}
