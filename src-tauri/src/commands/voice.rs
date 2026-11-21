//! Settings → Voice and the chat microphone.

use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app::blocking::with_app_state;
use crate::app::error::AppError;
use crate::app::events::DownloadProgressEvent;
use crate::app::state::AppState;
use crate::commands::cloud::{load_config, save_config};
use crate::config::VoiceConfig;
use crate::services::externals::{self, ToolStatus};
use crate::services::inference::openai_stream::{
    ChatTurn, Delta, Effort, initial_options, stream_with_retries,
};
use crate::services::inference::router::ChatTarget;
use crate::services::voice::live::{LiveEvent, LiveHub};
use crate::services::voice::{self, Choice, Delivery, ONLINE};

static LIVE: LazyLock<LiveHub> = LazyLock::new(LiveHub::default);

#[derive(Debug, Clone, Serialize)]
pub struct VoiceSettings {
    pub enabled: bool,
    pub model: String,
    pub local_only: bool,
    pub cleanup: bool,
    pub auto_send: bool,
    pub read_aloud: bool,
    pub voice_name: Option<String>,
    pub rate: f32,
    /// The end of each saved key, by provider. Keys never leave the backend.
    pub key_hints: BTreeMap<String, String>,
}

fn hint(key: &str) -> String {
    let k = key.trim();
    if k.len() > 8 {
        format!("••••{}", &k[k.len() - 4..])
    } else {
        "••••".into()
    }
}

fn dto(v: &VoiceConfig) -> VoiceSettings {
    VoiceSettings {
        enabled: v.enabled,
        model: v.model.clone(),
        local_only: v.local_only,
        cleanup: v.cleanup,
        auto_send: v.auto_send,
        read_aloud: v.read_aloud,
        voice_name: v.voice_name.clone(),
        rate: v.rate,
        key_hints: v
            .keys
            .iter()
            .filter(|(_, k)| !k.trim().is_empty())
            .map(|(p, k)| (p.clone(), hint(k)))
            .collect(),
    }
}

#[tauri::command]
pub fn voice_settings_get(state: State<'_, AppState>) -> Result<VoiceSettings, AppError> {
    Ok(dto(&load_config(&state)?.voice))
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct VoiceUpdate {
    pub enabled: Option<bool>,
    pub model: Option<String>,
    pub local_only: Option<bool>,
    pub cleanup: Option<bool>,
    pub auto_send: Option<bool>,
    pub read_aloud: Option<bool>,
    /// Some("") clears the choice (system default voice).
    pub voice_name: Option<String>,
    pub rate: Option<f32>,
}

/// Apply a settings change. Pure, so it is tested directly.
pub fn apply_voice_update(v: &mut VoiceConfig, u: VoiceUpdate) -> Result<(), AppError> {
    if let Some(m) = u.model {
        let choice = Choice::parse(&m)
            .ok_or_else(|| AppError::invalid_input("That is not a voice model."))?;
        let local_only = u.local_only.unwrap_or(v.local_only);
        if matches!(choice, Choice::Online { .. }) && local_only {
            return Err(AppError::invalid_input(
                "Turn off Local only to use an online voice model.",
            ));
        }
        v.model = m;
    }
    if let Some(lo) = u.local_only {
        v.local_only = lo;
        // Local only never keeps an online model selected.
        if lo && matches!(Choice::parse(&v.model), Some(Choice::Online { .. })) {
            v.model = VoiceConfig::default().model;
        }
    }
    if let Some(x) = u.enabled {
        v.enabled = x;
    }
    if let Some(x) = u.cleanup {
        v.cleanup = x;
    }
    if let Some(x) = u.auto_send {
        v.auto_send = x;
    }
    if let Some(x) = u.read_aloud {
        v.read_aloud = x;
    }
    if let Some(n) = u.voice_name {
        let n: String = n.trim().chars().take(200).collect();
        v.voice_name = (!n.is_empty()).then_some(n);
    }
    if let Some(r) = u.rate
        && r.is_finite()
    {
        v.rate = r.clamp(0.5, 2.0);
    }
    Ok(())
}

#[tauri::command]
pub fn voice_settings_set(
    state: State<'_, AppState>,
    update: VoiceUpdate,
) -> Result<VoiceSettings, AppError> {
    let mut cfg = load_config(&state)?;
    apply_voice_update(&mut cfg.voice, update)?;
    save_config(&state, &cfg)?;
    Ok(dto(&cfg.voice))
}

/// Save (or, with an empty key, remove) the key for an online voice provider.
/// Stored in the settings file like the chat provider keys.
#[tauri::command]
pub fn voice_key_set(
    state: State<'_, AppState>,
    provider: String,
    key: String,
) -> Result<VoiceSettings, AppError> {
    if !ONLINE.iter().any(|p| p.id == provider) {
        return Err(AppError::invalid_input("That is not a voice provider."));
    }
    let key = key.trim().to_string();
    if key.chars().any(|c| c.is_control() || c.is_whitespace()) || key.len() > 512 {
        return Err(AppError::invalid_input(
            "That key has spaces or characters a key cannot have.",
        ));
    }
    let mut cfg = load_config(&state)?;
    if key.is_empty() {
        cfg.voice.keys.remove(&provider);
    } else {
        cfg.voice.keys.insert(provider, key);
    }
    save_config(&state, &cfg)?;
    Ok(dto(&cfg.voice))
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalVoiceModel {
    pub id: String,
    pub name: String,
    pub params: String,
    pub description: String,
    pub license: String,
    pub size_bytes: u64,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct OnlineVoiceModel {
    /// The value stored as the chosen model.
    pub key: String,
    pub provider: &'static str,
    pub provider_label: &'static str,
    pub host: &'static str,
    pub key_url: &'static str,
    pub model: &'static str,
    pub label: &'static str,
    pub delivery: Delivery,
    pub note: &'static str,
    pub has_key: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceModels {
    pub local: Vec<LocalVoiceModel>,
    pub online: Vec<OnlineVoiceModel>,
    pub engine: ToolStatus,
}

#[tauri::command]
pub fn voice_models(state: State<'_, AppState>) -> Result<VoiceModels, AppError> {
    let cfg = load_config(&state)?;
    let mut local: Vec<LocalVoiceModel> = state
        .model_service
        .all_entries()
        .into_iter()
        .filter(|e| e.kind == "stt")
        .map(|e| LocalVoiceModel {
            installed: state.model_service.is_installed(&e.id)
                && e.companions
                    .iter()
                    .all(|c| state.model_service.models_dir().join(&c.dest).is_file()),
            size_bytes: e.file_size + e.companions.iter().map(|c| c.file_size).sum::<u64>(),
            id: e.id,
            name: e.name,
            params: e.params,
            description: e.description,
            license: e.license,
        })
        .collect();
    local.sort_by_key(|m| m.size_bytes);
    let online = ONLINE
        .iter()
        .flat_map(|p| {
            let has_key = cfg
                .voice
                .keys
                .get(p.id)
                .is_some_and(|k| !k.trim().is_empty());
            p.models.iter().map(move |m| OnlineVoiceModel {
                key: Choice::key(p.id, m.id, m.delivery),
                provider: p.id,
                provider_label: p.label,
                host: p.host,
                key_url: p.key_url,
                model: m.id,
                label: m.label,
                delivery: m.delivery,
                note: m.note,
                has_key,
            })
        })
        .collect();
    Ok(VoiceModels {
        local,
        online,
        engine: externals::status_of("sherpa-onnx"),
    })
}

/// Download the local voice engine (pinned build, checksum checked).
#[tauri::command]
pub async fn voice_engine_fetch(
    state: State<'_, AppState>,
    channel: tauri::ipc::Channel<DownloadProgressEvent>,
) -> Result<ToolStatus, AppError> {
    let pin = externals::sherpa_pin().ok_or_else(|| {
        AppError::invalid_input("There is no voice engine build for this computer.")
    })?;
    let (job_id, cancel) = state.jobs.create_job("download_voice_engine");
    let res = externals::fetch(pin, cancel, move |p| {
        let _ = channel.send(DownloadProgressEvent {
            model_id: "voice-engine".into(),
            downloaded_bytes: p.bytes_downloaded,
            total_bytes: p.bytes_total,
            percent: p.percent,
            speed_bps: p.speed_bps,
            status: "downloading".into(),
        });
    })
    .await;
    match res {
        Ok(_) => {
            state.jobs.complete_job(&job_id);
            Ok(externals::status_of("sherpa-onnx"))
        }
        Err(e) => {
            state.jobs.fail_job(&job_id, &e.message);
            Err(e)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Transcript {
    pub text: String,
    pub model: String,
    pub ms: u128,
    /// The audio left this computer.
    pub online: bool,
    pub language: Option<String>,
}

fn decode_b64(data: &str) -> Result<Vec<u8>, AppError> {
    base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| AppError::invalid_input("The recording could not be read."))
}

/// Transcribe a finished recording with the chosen (local or batch) model.
#[tauri::command]
pub async fn voice_transcribe(
    state: State<'_, AppState>,
    audio: String,
) -> Result<Transcript, AppError> {
    let wav = decode_b64(&audio)?;
    let range = voice::check_wav(&wav).map_err(AppError::invalid_input)?;
    let seconds = voice::pcm_seconds(range.len());
    if seconds < 0.25 {
        return Err(AppError::invalid_input(
            "The recording was too short. Hold the button while you speak.",
        ));
    }
    with_app_state(&state, move |state| {
        let cfg = load_config(&state)?;
        let choice = Choice::parse(&cfg.voice.model)
            .ok_or_else(|| AppError::invalid_input("Choose a voice model in Settings → Voice."))?;
        let started = Instant::now();
        match choice {
            Choice::Local(id) => {
                let r = voice::local::transcribe(&id, state.model_service.models_dir(), &wav)?;
                Ok(Transcript {
                    text: r.text,
                    model: id,
                    ms: r.ms,
                    online: false,
                    language: r.language,
                })
            }
            Choice::Online {
                provider,
                model,
                delivery,
            } => {
                if cfg.voice.local_only {
                    return Err(AppError::invalid_input(
                        "Local only is on, so online voice models are blocked.",
                    ));
                }
                if delivery == Delivery::Live {
                    return Err(AppError::invalid_input(
                        "This is a live model; it transcribes while you speak.",
                    ));
                }
                let key = cfg.voice.keys.get(&provider).cloned().unwrap_or_default();
                let text = voice::online::transcribe(&provider, &model, &key, &wav, seconds)?;
                Ok(Transcript {
                    text,
                    model: format!("{provider}:{model}"),
                    ms: started.elapsed().as_millis(),
                    online: true,
                    language: None,
                })
            }
        }
    })
    .await
}

/// Start a live session with the chosen live model. Audio follows with
/// `voice_live_push`; text comes back on the channel.
#[tauri::command]
pub fn voice_live_start(
    state: State<'_, AppState>,
    channel: tauri::ipc::Channel<LiveEvent>,
) -> Result<String, AppError> {
    let cfg = load_config(&state)?;
    if cfg.voice.local_only {
        return Err(AppError::invalid_input(
            "Local only is on, so online voice models are blocked.",
        ));
    }
    let Some(Choice::Online {
        provider,
        model,
        delivery: Delivery::Live,
    }) = Choice::parse(&cfg.voice.model)
    else {
        return Err(AppError::invalid_input(
            "The chosen voice model is not a live model.",
        ));
    };
    let key = cfg.voice.keys.get(&provider).cloned().unwrap_or_default();
    LIVE.start(
        &provider,
        &model,
        &key,
        Box::new(move |e| {
            let _ = channel.send(e);
        }),
    )
}

/// 16 kHz mono 16-bit PCM, base64.
#[tauri::command]
pub fn voice_live_push(session: String, pcm: String) -> Result<bool, AppError> {
    let bytes = decode_b64(&pcm)?;
    if bytes.len() > 16_000 * 2 * 5 {
        return Err(AppError::invalid_input(
            "Send audio in pieces of at most 5 seconds.",
        ));
    }
    Ok(LIVE.push(&session, bytes))
}

#[tauri::command]
pub fn voice_live_stop(session: String) -> bool {
    LIVE.stop(&session)
}

#[derive(Debug, Clone, Serialize)]
pub struct Cleanup {
    pub text: String,
    pub changed: bool,
    /// Why the text was left as it was, if it was.
    pub skipped: Option<String>,
}

/// AI cleanup: the loaded local model removes filler words and fixes
/// punctuation. Only text goes to it, and only on this computer.
#[tauri::command]
pub async fn voice_cleanup(state: State<'_, AppState>, text: String) -> Result<Cleanup, AppError> {
    let original: String = text.chars().take(8000).collect();
    if original.trim().is_empty() {
        return Ok(Cleanup {
            text: original,
            changed: false,
            skipped: None,
        });
    }
    if state.inference_engine.local().endpoint().is_none() {
        return Ok(Cleanup {
            text: original,
            changed: false,
            skipped: Some(
                "No model is loaded on this computer, so the text was not tidied.".into(),
            ),
        });
    }
    let engine = state.inference_engine.clone();
    let input = original.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result = (|| {
            let endpoint = engine.endpoint_for(&ChatTarget::Local)?;
            let mut turns = vec![
                ChatTurn::System(
                    "You tidy dictated text. Remove filler words (um, uh, er, like, you know), stutters and repeated words; fix punctuation and capital letters. Keep the words, meaning and language. Do not answer, translate or add anything. Reply with the tidied text only.".into(),
                ),
                ChatTurn::User { text: input.clone(), images: Vec::new() },
            ];
            let budget = (input.len() / 3 + 64).min(2048) as i32;
            let mut opts = initial_options(endpoint.flavor, budget, 0.1, Vec::new(), Effort::Off);
            let cancel = AtomicBool::new(false);
            let deadline = Instant::now() + Duration::from_secs(45);
            let mut out = String::new();
            stream_with_retries(
                engine.chat_client(),
                &endpoint,
                &mut turns,
                &mut opts,
                &cancel,
                &mut |d| {
                    if let Delta::Content(t) = d {
                        out.push_str(&t);
                    }
                    if Instant::now() > deadline {
                        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                },
                &mut |_| {},
            )?;
            Ok::<_, AppError>(out)
        })();
        let _ = tx.send(result);
    });
    match rx
        .await
        .unwrap_or_else(|_| Err(AppError::internal("Cleanup stopped.")))
    {
        Ok(out) => {
            let cleaned = out.trim().trim_matches('"').trim().to_string();
            // A reply far longer or shorter than the input is not a tidy-up.
            let ratio = cleaned.chars().count() as f32 / original.chars().count().max(1) as f32;
            if cleaned.is_empty() || !(0.4..=1.6).contains(&ratio) {
                return Ok(Cleanup {
                    text: original,
                    changed: false,
                    skipped: Some(
                        "The model's version did not look like a tidy-up, so your words were kept."
                            .into(),
                    ),
                });
            }
            let changed = cleaned != original.trim();
            Ok(Cleanup {
                text: cleaned,
                changed,
                skipped: None,
            })
        }
        Err(e) => Ok(Cleanup {
            text: original,
            changed: false,
            skipped: Some(format!("Cleanup failed: {}", e.message)),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_only_blocks_and_resets_online_models() {
        let mut v = VoiceConfig::default();
        assert!(v.local_only);
        let err = apply_voice_update(
            &mut v,
            VoiceUpdate {
                model: Some("online:groq:whisper-large-v3-turbo:batch".into()),
                ..Default::default()
            },
        );
        assert!(err.is_err());
        apply_voice_update(
            &mut v,
            VoiceUpdate {
                local_only: Some(false),
                model: Some("online:groq:whisper-large-v3-turbo:batch".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(v.model, "online:groq:whisper-large-v3-turbo:batch");
        apply_voice_update(
            &mut v,
            VoiceUpdate {
                local_only: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(v.model, "stt-whisper-tiny");
        apply_voice_update(
            &mut v,
            VoiceUpdate {
                rate: Some(9.0),
                voice_name: Some(" ".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(v.rate, 2.0);
        assert_eq!(v.voice_name, None);
        assert!(
            apply_voice_update(
                &mut v,
                VoiceUpdate {
                    model: Some("qwen3".into()),
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
