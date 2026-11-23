use crate::app::blocking::{run_blocking, with_app_state};
use crate::app::error::AppError;
use crate::app::events::ChatStreamEvent;
use crate::app::paths::conversations_dir;
use crate::app::state::AppState;
use crate::domain::chat::{
    ChatAttachment, ChatMessage, ChatSteerRequest, ChatStreamRequest, ChatTargetRequest,
    ChatThread, SummarizeRequest, SummarizeResponse, ThreadExportRequest, ThreadExportResponse,
    ToolDecisionRequest,
};
use crate::services::agent::AgentRun;
use crate::services::agent::approval::Decision;
use crate::services::agent::tools::Permissions;
use crate::services::inference::InferenceEngine;
use crate::services::inference::PromptMessage;
use crate::services::inference::capabilities::effort_for_request;
use crate::services::inference::context::ContextBudget;
use crate::services::inference::modes::{
    ChatMode, ToolGroups, compose_system_prompt, tool_guidance,
};
use crate::services::inference::openai_stream::{ChatTurn, Effort};
use crate::services::inference::router::ChatTarget;
use std::fs;
use std::sync::atomic::Ordering;
use tauri::State;

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredThread {
    pub id: String,
    pub title: String,
    pub system_prompt: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

const MAX_IMAGES: usize = 4;
const MAX_IMAGES_BYTES: usize = 20 * 1024 * 1024;
/// Reject unsupported or oversized attachments before any model work starts.
fn validate_images(images: &[String]) -> Result<(), AppError> {
    if images.len() > MAX_IMAGES {
        return Err(AppError::invalid_input(format!(
            "Attach at most {MAX_IMAGES} images."
        )));
    }
    const PREFIXES: [&str; 4] = [
        "data:image/png;base64,",
        "data:image/jpeg;base64,",
        "data:image/webp;base64,",
        "data:image/gif;base64,",
    ];
    let mut total = 0usize;
    for img in images {
        if !PREFIXES.iter().any(|p| img.starts_with(p)) {
            return Err(AppError::invalid_input(
                "Only PNG, JPEG, WEBP and GIF images are supported.",
            ));
        }
        total += img.len();
    }
    if total > MAX_IMAGES_BYTES {
        return Err(AppError::invalid_input(
            "The attached images are too large. Keep them under 20 MB in total.",
        ));
    }
    Ok(())
}

/// Thread ids become file names in the conversations folder, so only a short
/// plain token is accepted. The app issues ids like `t_1731542400000`.
fn validate_thread_id(thread_id: &str) -> Result<(), AppError> {
    let plain = !thread_id.is_empty()
        && thread_id.len() <= 64
        && thread_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if plain {
        Ok(())
    } else {
        Err(AppError::invalid_input(
            "That conversation id is not valid.",
        ))
    }
}

/// What the chat screen needs to know about the local engine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InferenceStatusDto {
    pub loaded: bool,
    pub loading: bool,
}

#[tauri::command]
pub fn inference_status(state: State<'_, AppState>) -> InferenceStatusDto {
    InferenceStatusDto {
        loaded: state.inference_engine.status().loaded,
        loading: state.engine_loading.load(Ordering::SeqCst),
    }
}

#[tauri::command]
pub async fn inference_load(state: State<'_, AppState>, id: String) -> Result<String, AppError> {
    load_and_activate(&state, id).await
}

/// Load a model into the local engine, then mark it active and remember it in
/// the selection. The "Use this" button, the chat picker and the preset install
/// all come through here.
pub(crate) async fn load_and_activate(state: &AppState, id: String) -> Result<String, AppError> {
    // Filesystem truth: the model must exist in the active catalog or the
    // community registry AND on disk. This keeps freshly downloaded models
    // loadable and honors split-GGUF dest names.
    state
        .model_service
        .get_model_entry(&id)
        .ok_or_else(|| AppError::model_not_installed(&id))?;
    let path = state.model_service.model_file_path(&id);
    if !path.is_file() {
        return Err(AppError::model_not_installed(&id));
    }
    // The first load downloads the model engine for this processor.
    crate::services::engine::ensure().await?;
    // The engine uses a blocking HTTP client. Run it on a plain thread, not the async runtime.
    let engine = state.inference_engine.clone();
    let load_path = path.clone();
    // Context is the user's setting capped by the model's native window.
    let context = {
        let setting = state
            .config_store
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))
            .and_then(|s| s.load().map_err(AppError::internal))
            .map(|c| c.context_tokens)
            .unwrap_or(8192);
        state.model_service.effective_context(&id, setting)
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(engine.load(&load_path, context));
    });
    let res = rx.await.map_err(|e| AppError::internal(e.to_string()))??;
    if let Err(e) = state.model_manager.activate(&id) {
        log::warn!("Could not save the active model {id}: {e}");
    }

    // Persist active model in config for auto-reload
    if let Ok(store) = state.config_store.lock()
        && let Ok(mut cfg) = store.load()
    {
        if !cfg.selected_models.contains(&id) {
            cfg.selected_models.push(id.clone());
        }
        let _ = store.save(&cfg);
    }

    Ok(res)
}

/// Stop the local engine, free the model's RAM and forget the active model.
/// The model file stays installed. Loading it again restarts the engine.
#[tauri::command]
pub async fn inference_unload(state: State<'_, AppState>) -> Result<(), AppError> {
    with_app_state(&state, |state| {
        state.inference_engine.unload()?;
        if let Err(e) = state.model_manager.clear_active() {
            log::warn!("Could not clear the active model: {e}");
        }
        Ok(())
    })
    .await
}

/// A message as the model reads it: the text, then each attached file.
pub(crate) fn history_text(content: &str, attachments: &[ChatAttachment]) -> String {
    history_text_within(content, attachments, None)
}

/// Newest history messages that always keep their attached files whole.
const KEEP_FULL_FILES: usize = 4;
/// Characters kept of a file attached to an older message when the history
/// runs short of room.
const OLD_FILE_CHARS: usize = 2_000;

/// [`history_text`], with each attached file cut to `cap` characters when
/// one is given. A cut file says so, so the model knows it saw only the start.
pub(crate) fn history_text_within(
    content: &str,
    attachments: &[ChatAttachment],
    cap: Option<usize>,
) -> String {
    let mut out = content.to_string();
    for file in attachments {
        let Some(text) = file.text.as_deref().filter(|_| file.kind == "text") else {
            continue;
        };
        let name = file.name.replace('"', "'");
        let (text, cut) = match cap {
            Some(cap) if text.chars().count() > cap => (
                format!(
                    "{}\n[The rest of this file was left out to save space. Ask the user to attach it again if you need it.]",
                    text.chars().take(cap).collect::<String>()
                ),
                true,
            ),
            _ => (text.to_string(), false),
        };
        out.push_str(&format!(
            "\n\n<attached_file name=\"{name}\"{}>\n{text}\n</attached_file>",
            if file.truncated || cut {
                " shortened=\"yes\""
            } else {
                ""
            }
        ));
    }
    out
}

/// The model this message goes to: the picker's choice, or the default.
pub(crate) fn resolve_target(
    state: &AppState,
    cfg: &crate::config::AppConfig,
    requested: Option<&ChatTargetRequest>,
) -> Result<(ChatTarget, ChatTargetRequest, String), AppError> {
    if let Some(req) = requested
        && let Some(pid) = req.provider_id.as_deref()
    {
        if !cfg.online_allowed() {
            return Err(AppError::invalid_input(
                "Online providers are switched off in Settings, so this message was not sent. Pick a model on this computer, or switch them on.",
            ));
        }
        let provider = cfg
            .cloud_providers
            .iter()
            .find(|p| p.id == pid && p.enabled)
            .ok_or_else(|| {
                AppError::invalid_input(
                    "That online provider is switched off or removed. Pick another model.",
                )
            })?;
        let target = crate::commands::cloud::remote_target(provider, Some(&req.model));
        let label = format!("{} · {}", target.model, provider.label);
        return Ok((ChatTarget::Remote(target), req.clone(), label));
    }
    match state.inference_engine.current_target() {
        ChatTarget::Remote(t) if requested.is_none() && cfg.online_allowed() => {
            let provider_id = cfg.active_cloud_provider.clone();
            let label = format!("{} · {}", t.model, t.label);
            let req = ChatTargetRequest {
                provider_id,
                model: t.model.clone(),
            };
            Ok((ChatTarget::Remote(t), req, label))
        }
        _ => {
            let id = state.model_manager.active_model_id().unwrap_or_default();
            let label = state
                .model_service
                .get_model_entry(&id)
                .map(|e| e.name)
                .unwrap_or_else(|| "Local model".to_string());
            if state.inference_engine.local().endpoint().is_none() {
                return Err(AppError::model_not_installed(
                    "No model is loaded. Pick a model in the model menu.",
                ));
            }
            Ok((
                ChatTarget::Local,
                ChatTargetRequest {
                    provider_id: None,
                    model: id,
                },
                label,
            ))
        }
    }
}

#[tauri::command]
pub async fn chat_stream(
    state: State<'_, AppState>,
    req: ChatStreamRequest,
    channel: tauri::ipc::Channel<ChatStreamEvent>,
) -> Result<String, AppError> {
    let (run, model_label, job_id, cancel_flag) = prepare_run(&state, req, "chat")?;
    let owned: AppState = (*state).clone();
    let jobs = state.jobs.clone();
    let jid = job_id.clone();
    let _ = channel.send(ChatStreamEvent::Status {
        label: format!("Answering with {model_label}"),
    });

    std::thread::spawn(move || {
        let result = crate::services::agent::run(&owned, run, &cancel_flag, &channel);
        match result {
            Ok(done) => {
                jobs.complete_job(&jid);
                let _ = channel.send(ChatStreamEvent::Done {
                    finish_reason: if done.cancelled {
                        "cancelled".to_string()
                    } else {
                        done.finish_reason
                    },
                    total_tokens: done.tokens,
                    prompt_tokens: done.prompt_tokens,
                    cache_tokens: done.cache_tokens,
                    first_prompt_tokens: done.first_prompt_tokens,
                    system_tokens: done.system_tokens,
                    tool_tokens: done.tool_tokens,
                });
            }
            Err(e) => {
                jobs.fail_job(&jid, &e.message);
                let _ = channel.send(ChatStreamEvent::Error { error: e.message });
            }
        }
    });

    Ok(job_id)
}

/// Everything an answer needs, from a chat request: the model, the tools,
/// the system prompt and the history. `source` is "chat" in the app and
/// "share" for a shared link, which always asks before tools change
/// anything.
pub(crate) fn prepare_run(
    state: &AppState,
    mut req: ChatStreamRequest,
    source: &'static str,
) -> Result<
    (
        AgentRun,
        String,
        String,
        std::sync::Arc<std::sync::atomic::AtomicBool>,
    ),
    AppError,
> {
    if source == "share" {
        // Shared links always ask, stay in the shared workspace and never
        // change the owner's skills.
        req.approval_mode = None;
        req.folder = None;
        req.learn_skills = Some(false);
    }
    let jail = folder_jail(state, req.folder.as_deref())?;
    validate_images(&req.images)?;
    let cfg = {
        let store = state
            .config_store
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        store.load().map_err(AppError::internal)?
    };
    let (target, target_req, model_label) = resolve_target(state, &cfg, req.target.as_ref())?;
    let caps = crate::commands::picker::caps_for_target(state, &cfg, &target_req);

    let mode = ChatMode::parse(req.mode.as_deref());
    let temperature = mode.temperature(req.temperature.unwrap_or(0.7));
    let answer_tokens = req.max_tokens.unwrap_or(2048).clamp(64, 32_768);
    let effort = Effort::parse(effort_for_request(req.effort.as_deref()).as_deref());
    let thinks = caps.reasoning && !effort.is_off();
    // Only send an effort the model understands.
    let effort = if caps.reasoning { effort } else { Effort::Auto };

    let modules = &cfg.developer.modules;
    let module_on = |id: &str| modules.get(id).copied().unwrap_or(true);
    let wanted_mcp = req.mcp_servers.clone();
    let mcp: Vec<_> = if module_on("mcp") {
        cfg.mcp_servers
            .iter()
            .filter(|s| s.enabled)
            .filter(|s| wanted_mcp.as_ref().is_none_or(|ids| ids.contains(&s.id)))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    let perms = Permissions {
        web: req.use_web.unwrap_or(false) && module_on("web_in_chat"),
        notes: req.use_recall.unwrap_or(true) && module_on("memory"),
        files: req.use_files.unwrap_or(true),
        skills: req.use_skills.unwrap_or(true) && module_on("skills"),
        shell: req.use_shell.unwrap_or(true) && module_on("terminal"),
        // Autonomous mode works on without asking follow-up questions.
        ask: mode != ChatMode::Autonomous,
        todos: true,
        subagents: req.use_subagents.unwrap_or(true),
        learn_skills: req.learn_skills.unwrap_or(true)
            && req.use_skills.unwrap_or(true)
            && module_on("skills"),
        mcp,
    };
    let approval_mode = crate::services::agent::ApprovalMode::parse(req.approval_mode.as_deref());

    let context_tokens = match &target {
        ChatTarget::Local => state
            .inference_engine
            .local()
            .endpoint()
            .map(|ep| ep.context_size.max(1024) as usize)
            .unwrap_or(4096),
        ChatTarget::Remote(_) => caps.context.unwrap_or(16_384).clamp(4096, 262_144) as usize,
    };

    // The system prompt: the user's prompt, the mode and how to use the
    // tools this chat has. It stays the same from one message to the next,
    // so the engine can reuse its cached prompt instead of reading the whole
    // chat again; the clock, which changes every minute, rides on the new
    // message instead (below).
    let mut extra = Vec::new();
    let guidance = tool_guidance(
        ToolGroups {
            web: perms.web,
            notes: perms.notes,
            files: perms.files,
            skills: perms.skills,
            shell: perms.shell,
            mcp: !perms.mcp.is_empty(),
        },
        mode,
    );
    if caps.tools && !guidance.is_empty() {
        extra.push(guidance);
    }
    if let Some(folder) = req.folder.as_deref().filter(|f| !f.trim().is_empty()) {
        extra.push(format!(
            "This chat works in the user's project folder {folder}. File tools and terminal commands use it; paths are relative to it."
        ));
    }
    if approval_mode == crate::services::agent::ApprovalMode::Plan {
        extra.push(PLAN_GUIDANCE.to_string());
    }
    if req.rich_output.unwrap_or(true) {
        extra.push(RICH_OUTPUT_GUIDANCE.to_string());
        // Interactive forms (A2UI) come with the other visuals.
        extra.push(A2UI_GUIDANCE.to_string());
    }
    // Project rules from AGENTS.md / CLAUDE.md / .hermes.md / .cursorrules,
    // best-effort and capped. Users can switch this module off in Developer.
    let context_files_on = state
        .config_store
        .lock()
        .ok()
        .and_then(|store| store.load().ok())
        .map(|cfg| cfg.developer.module_on("context_files"))
        .unwrap_or(true);
    if context_files_on {
        let mut roots = vec![crate::app::paths::workspace_dir()];
        roots.extend(crate::services::context_files::indexed_roots(
            &crate::app::paths::index_dir().join("library.db"),
        ));
        if let Some(rules) = crate::services::inference::context::project_rules(&roots, 8192) {
            extra.push(rules);
        }
    }
    let system = compose_system_prompt(req.system_prompt.as_deref(), mode, &extra.join("\n\n"));

    let thinking_room = if thinks {
        effort.thinking_allowance()
    } else {
        0
    };
    let budget = ContextBudget::new(context_tokens, (answer_tokens + thinking_room) as usize);

    // History the model reads: failed answers are left out, attached files
    // are written out. When the history would take more than half the room,
    // files attached to older messages are shortened before whole turns
    // have to go; the newest messages keep their files whole.
    let sent: Vec<&ChatMessage> = req
        .history
        .iter()
        .filter(|m| m.error != Some(true) && !m.content.trim().is_empty())
        .collect();
    let build_history = |cap_old: bool| -> Vec<ChatMessage> {
        let keep_from = sent.len().saturating_sub(KEEP_FULL_FILES);
        sent.iter()
            .enumerate()
            .map(|(i, m)| ChatMessage {
                role: m.role.clone(),
                content: history_text_within(
                    &m.content,
                    m.attachments.as_deref().unwrap_or(&[]),
                    (cap_old && i < keep_from).then_some(OLD_FILE_CHARS),
                ),
                ..Default::default()
            })
            .collect()
    };
    let mut history = build_history(false);
    let room = context_tokens.saturating_sub((answer_tokens + thinking_room) as usize);
    let history_tokens: usize = history
        .iter()
        .map(|m| ContextBudget::estimate_tokens(&m.content))
        .sum();
    if history_tokens > room / 2 {
        history = build_history(true);
    }
    let mut message = history_text(&req.message, &req.attachments);
    if let Some(now) = req.now.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        let now: String = now.chars().take(120).collect();
        message.push_str(&format!("\n\n[The user's current date and time: {now}.]"));
    }
    let turns: Vec<ChatTurn> = budget
        .build_messages(system.as_deref(), &history, &message, &req.images)
        .into_iter()
        .map(|m| match m.role.as_str() {
            "system" => ChatTurn::System(m.content),
            "assistant" => ChatTurn::Assistant {
                text: m.content,
                tool_calls: Vec::new(),
            },
            _ => ChatTurn::User {
                text: m.content,
                images: m.images,
            },
        })
        .collect();

    let (job_id, cancel_flag) = state.jobs.create_job("inference_chat");
    let run = AgentRun {
        job_id: job_id.clone(),
        target,
        turns,
        answer_tokens,
        temperature,
        effort,
        thinks,
        // A model known to have no tools still gets web results and notes,
        // gathered before it answers.
        use_tools: caps.tools,
        perms,
        max_rounds: mode.max_rounds(),
        context_tokens,
        auto_approve: req.allowed_tools.iter().cloned().collect(),
        approval_mode,
        source,
        first_citation: 1,
        user_message: req.message.clone(),
        read_top_page: mode.gathers_web(),
        jail,
        folder: req.folder.clone(),
        session: req.thread_id.clone(),
    };
    Ok((run, model_label, job_id, cancel_flag))
}

const PLAN_GUIDANCE: &str = "Plan mode is on. Look around with the read-only tools as much as you need, then answer with a short numbered plan: what you would change, in which files, and which commands you would run. Do not try to make the changes; they will not run. End by saying the user can press Build to carry out the plan.";

const RICH_OUTPUT_GUIDANCE: &str = "The chat can draw visuals from fenced code blocks. Use one only when a visual helps more than text:\n- ```chart with JSON: {\"type\": \"bar\" | \"line\" | \"pie\", \"title\": \"...\", \"labels\": [\"...\"], \"series\": [{\"name\": \"...\", \"values\": [1, 2]}]}\n- ```mermaid for diagrams (flowchart, sequence, class, state, timeline, gantt).\n- ```html for an interface mockup or a small page. It runs in a sandbox with no network and no access to the user's files; inline all CSS and scripts. To let the user pick an option on it, call localaide.choose(\"the choice in a few words\") from a button; the choice comes back as the user's next message.\nFor tables, use markdown tables.";

/// How to draw an interactive form with A2UI v1.0 (Basic Catalog). Kept
/// short: the components, binding, checks and actions the chat renders.
const A2UI_GUIDANCE: &str = "When the user should fill in, choose or press something (a form, a checklist, a quiz, a booking, settings to pick), draw it with A2UI 1.0 in a ```a2ui block: one JSON message per line. Start with {\"version\":\"1.0\",\"createSurface\":{\"surfaceId\":\"booking_1\",\"catalogId\":\"https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json\",\"components\":[...],\"dataModel\":{...}}}. Components are a flat list; each has a unique \"id\" (letters, digits and underscores) and a \"component\"; exactly one has the id \"root\".\n- Layout: Column/Row {\"children\":[\"id1\",\"id2\"],\"justify\",\"align\"} (or a template {\"children\":{\"path\":\"/items\",\"componentId\":\"item_tpl\"}}, where the template's paths are relative, like \"name\"), List, Card {\"child\":\"id\"}, Tabs {\"tabs\":[{\"title\":\"...\",\"child\":\"id\"}]}, Modal {\"trigger\":\"id\",\"content\":\"id\"}, Divider. Leaves: Text {\"text\":\"markdown\",\"variant\":\"body\"|\"caption\"}, Icon {\"name\":\"check\"}.\n- Inputs bind to the data model with {\"@path\":\"/form/name\"}: TextField {\"label\",\"value\",\"variant\":\"shortText\"|\"longText\"|\"number\"|\"obscured\"}, CheckBox {\"label\",\"value\"}, ChoicePicker {\"label\",\"options\":[{\"label\",\"value\"}],\"value\":{\"@path\":\"/form/size\"},\"variant\":\"mutuallyExclusive\"|\"multipleSelection\",\"displayStyle\":\"checkbox\"|\"chips\"} (its value is a list of strings), Slider {\"label\",\"min\",\"max\",\"value\"}, DateTimeInput {\"label\",\"value\",\"enableDate\":true,\"enableTime\":false}.\n- Button {\"child\":\"label_text_id\",\"variant\":\"primary\",\"action\":{\"event\":{\"name\":\"submit\",\"context\":{\"name\":{\"@path\":\"/form/name\"}}}}}. Add \"@checks\":[{\"condition\":{\"@call\":\"required\",\"args\":{\"value\":{\"@path\":\"/form/name\"}}},\"message\":\"Enter a name.\"}] to an input or button (a failing check disables the button). Functions: required, email, length {min,max}, numeric {min,max}, regex {pattern}, and/or {values}, not, formatString {\"value\":\"Hi ${/form/name}\"}.\nA pressed button comes back as the user's next message with a ```a2ui-event block holding an A2UI action (name, surfaceId, context). Reply to it in words, or change the same surface with {\"version\":\"1.0\",\"updateDataModel\":{\"surfaceId\":\"booking_1\",\"path\":\"/form/status\",\"value\":\"Booked\"}} or updateComponents; reuse a surfaceId only to update it. Images load only from data: URLs. Use a form only when it helps more than plain text.";

/// The jail for the folder a chat works in. A project folder must be a
/// real folder the user picked; the whole disk, the home folder itself and
/// LocalAide's own data folder (keys, settings, chats) are refused.
pub(crate) fn folder_jail(
    state: &AppState,
    folder: Option<&str>,
) -> Result<std::sync::Arc<crate::services::tools::sandbox::WorkspaceJail>, AppError> {
    let Some(folder) = folder.map(str::trim).filter(|f| !f.is_empty()) else {
        return Ok(state.tools.workspace_jail());
    };
    let path = std::path::Path::new(folder);
    if !path.is_absolute() || !path.is_dir() {
        return Err(AppError::invalid_input(format!(
            "The chat's folder {folder} is not there any more. Start a chat in another folder."
        )));
    }
    let real = fs::canonicalize(path).map_err(|e| AppError::storage(e.to_string()))?;
    let home = dirs_next::home_dir().and_then(|h| fs::canonicalize(h).ok());
    if real.parent().is_none() || home.as_deref() == Some(real.as_path()) {
        return Err(AppError::invalid_input(
            "Pick a project folder, not the whole disk or your home folder.",
        ));
    }
    if let Ok(app) = fs::canonicalize(crate::app::paths::app_dir())
        && (real.starts_with(&app) || app.starts_with(&real))
        && fs::canonicalize(crate::app::paths::workspace_dir()).ok() != Some(real.clone())
    {
        return Err(AppError::invalid_input(
            "That folder holds LocalAide's own data, so a chat cannot work in it.",
        ));
    }
    Ok(std::sync::Arc::new(
        crate::services::tools::sandbox::WorkspaceJail::new(&real)?,
    ))
}

/// The user's answer to a tool approval question.
#[tauri::command]
pub fn chat_tool_decide(
    state: State<'_, AppState>,
    req: ToolDecisionRequest,
) -> Result<bool, AppError> {
    let decision = Decision::parse(&req.decision)
        .ok_or_else(|| AppError::invalid_input("Answer allow, allow_chat or deny."))?;
    Ok(state.agent.approvals.decide(&req.approval_id, decision))
}

/// A note for an answer that is still running; the model reads it before
/// its next step. False when the answer has already finished, so the chat
/// sends the note as its next message instead.
#[tauri::command]
pub fn chat_steer(state: State<'_, AppState>, req: ChatSteerRequest) -> Result<bool, AppError> {
    let text = req.text.trim();
    if text.is_empty() {
        return Err(AppError::invalid_input("Write the note first."));
    }
    let text: String = text.chars().take(8_000).collect();
    let running = state
        .jobs
        .get_job(&req.job_id)
        .is_some_and(|job| job.state == crate::app::jobs::JobState::Running);
    if !running {
        return Ok(false);
    }
    state.agent.steer(&req.job_id, text);
    Ok(true)
}

#[tauri::command]
pub fn chat_cancel(state: State<'_, AppState>, job_id: String) -> Result<(), AppError> {
    state.jobs.cancel_job(&job_id);
    Ok(())
}

#[tauri::command]
pub async fn summarize(
    state: State<'_, AppState>,
    req: SummarizeRequest,
) -> Result<SummarizeResponse, AppError> {
    let status = state.inference_engine.status();
    if !status.loaded {
        return Err(AppError::model_not_installed(
            "No model loaded for summarization.",
        ));
    }

    let system_prompt =
        "You are a concise summarizer. Summarize the following text in a few clear sentences:";
    let messages = vec![
        PromptMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
            images: Vec::new(),
        },
        PromptMessage {
            role: "user".to_string(),
            content: req.text.clone(),
            images: Vec::new(),
        },
    ];
    let max_tokens = req.max_words.unwrap_or(256) as i32;

    let (_job_id, cancel_flag) = state.jobs.create_job("summarize");
    let accumulated = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let acc_cb = accumulated.clone();

    // Run on a plain thread: the engine uses a blocking HTTP client.
    let engine = state.inference_engine.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result = engine.generate(
            &messages,
            max_tokens,
            0.3,
            cancel_flag,
            Box::new(move |tok| {
                if let Ok(mut lock) = acc_cb.lock() {
                    lock.push_str(&tok.token);
                }
            }),
        );
        let _ = tx.send(result.map(|_| ()));
    });
    rx.await.map_err(|e| AppError::internal(e.to_string()))??;

    let summary = accumulated
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Ok(SummarizeResponse { summary })
}

#[tauri::command]
pub async fn chat_thread_list() -> Result<Vec<ChatThread>, AppError> {
    run_blocking(chat_thread_list_work).await
}

pub(crate) fn chat_thread_list_work() -> Result<Vec<ChatThread>, AppError> {
    let dir = conversations_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut list = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && let Ok(content) = fs::read_to_string(&path)
            && let Ok(stored) = serde_json::from_str::<StoredThread>(&content)
        {
            list.push(ChatThread {
                id: stored.id,
                title: stored.title,
                system_prompt: stored.system_prompt,
                created_at: stored.created_at,
                updated_at: stored.updated_at,
                message_count: stored.messages.len(),
                folder: stored.folder,
            });
        }
    }
    list.sort_by_key(|t| std::cmp::Reverse(t.updated_at));
    Ok(list)
}

#[tauri::command]
pub async fn chat_thread_save(
    thread_id: String,
    title: String,
    system_prompt: Option<String>,
    messages: Vec<ChatMessage>,
    folder: Option<String>,
) -> Result<(), AppError> {
    run_blocking(move || chat_thread_save_work(thread_id, title, system_prompt, messages, folder))
        .await
}

/// `folder` None keeps the folder the saved chat already has.
pub(crate) fn chat_thread_save_work(
    thread_id: String,
    title: String,
    system_prompt: Option<String>,
    messages: Vec<ChatMessage>,
    folder: Option<String>,
) -> Result<(), AppError> {
    validate_thread_id(&thread_id)?;
    let dir = conversations_dir();
    fs::create_dir_all(&dir)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let path = dir.join(format!("{thread_id}.json"));
    let existing: Option<StoredThread> = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());
    let created_at = existing.as_ref().map(|t| t.created_at).unwrap_or(now);
    let folder = folder
        .filter(|f| !f.trim().is_empty())
        .or_else(|| existing.and_then(|t| t.folder));

    let stored = StoredThread {
        id: thread_id,
        title,
        system_prompt,
        created_at,
        updated_at: now,
        messages,
        folder,
    };

    let json = serde_json::to_string_pretty(&stored).map_err(AppError::from)?;
    fs::write(&path, json)?;
    Ok(())
}

#[tauri::command]
pub async fn chat_thread_load(thread_id: String) -> Result<Vec<ChatMessage>, AppError> {
    run_blocking(move || chat_thread_load_work(thread_id)).await
}

pub(crate) fn chat_thread_load_work(thread_id: String) -> Result<Vec<ChatMessage>, AppError> {
    validate_thread_id(&thread_id)?;
    let path = conversations_dir().join(format!("{thread_id}.json"));
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(&path)?;
    let stored: StoredThread = serde_json::from_str(&data).map_err(AppError::from)?;
    Ok(stored.messages)
}

#[tauri::command]
pub async fn chat_thread_delete(thread_id: String) -> Result<(), AppError> {
    run_blocking(move || chat_thread_delete_work(thread_id)).await
}

pub(crate) fn chat_thread_delete_work(thread_id: String) -> Result<(), AppError> {
    validate_thread_id(&thread_id)?;
    let path = conversations_dir().join(format!("{thread_id}.json"));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Asks where to save the chat, then writes it there. Returns the saved path,
/// or `None` when the user closes the save window.
#[tauri::command]
pub async fn chat_thread_export_save(
    app: tauri::AppHandle,
    req: ThreadExportRequest,
) -> Result<Option<String>, AppError> {
    use tauri_plugin_dialog::DialogExt;

    run_blocking(move || {
        let export = chat_thread_export_work(req)?;
        let (label, extension) = if export.filename.ends_with(".json") {
            ("JSON", "json")
        } else {
            ("Markdown", "md")
        };
        // The save window blocks until the user answers, so this runs on a
        // worker thread.
        let Some(picked) = app
            .dialog()
            .file()
            .set_title("Save chat")
            .set_file_name(&export.filename)
            .add_filter(label, &[extension])
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = picked
            .into_path()
            .map_err(|e| AppError::invalid_input(format!("That location cannot be used: {e}")))?;
        fs::write(&path, export.content)
            .map_err(|e| AppError::storage(format!("The chat could not be saved there: {e}")))?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}

/// Largest export the window may hand over (Markdown of a very long chat).
const MAX_EXPORT_BYTES: usize = 32 * 1024 * 1024;

/// Asks where to save, then writes text the window already formatted (the
/// same Markdown "Copy full conversation" uses). The path comes only from
/// the save window the user answered; returns None when they close it.
#[tauri::command]
pub async fn chat_export_text_save(
    app: tauri::AppHandle,
    file_name: String,
    content: String,
) -> Result<Option<String>, AppError> {
    use tauri_plugin_dialog::DialogExt;

    if content.len() > MAX_EXPORT_BYTES {
        return Err(AppError::invalid_input(
            "This chat is too large to save as one file.",
        ));
    }
    let stem = export_file_stem(file_name.trim_end_matches(".md"));
    let suggested = format!("{stem}.md");
    run_blocking(move || {
        let Some(picked) = app
            .dialog()
            .file()
            .set_title("Save chat")
            .set_file_name(&suggested)
            .add_filter("Markdown", &["md"])
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = picked
            .into_path()
            .map_err(|e| AppError::invalid_input(format!("That location cannot be used: {e}")))?;
        fs::write(&path, content)
            .map_err(|e| AppError::storage(format!("The chat could not be saved there: {e}")))?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}

/// Turns a chat title into a file name that every system accepts.
fn export_file_stem(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(60)
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "chat".to_string()
    } else {
        trimmed.to_string()
    }
}

fn chat_thread_export_work(req: ThreadExportRequest) -> Result<ThreadExportResponse, AppError> {
    validate_thread_id(&req.thread_id)?;
    let path = conversations_dir().join(format!("{}.json", req.thread_id));
    if !path.exists() {
        return Err(AppError::invalid_input(
            "Save the chat first. It has no messages yet.",
        ));
    }
    let data = fs::read_to_string(&path)?;
    let stored: StoredThread = serde_json::from_str(&data).map_err(AppError::from)?;
    let stem = export_file_stem(&stored.title);

    if req.format.to_lowercase() == "json" {
        Ok(ThreadExportResponse {
            content: data,
            filename: format!("{stem}.json"),
        })
    } else {
        // Markdown format
        let mut md = format!("# {}\n\n", stored.title);
        if let Some(sys) = stored.system_prompt
            && !sys.trim().is_empty()
        {
            md.push_str(&format!("**System Prompt:** {}\n\n---\n\n", sys));
        }
        for msg in stored.messages {
            let role_label = if msg.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            md.push_str(&format!("### {}\n\n{}\n\n", role_label, msg.content));
        }
        Ok(ThreadExportResponse {
            content: md,
            filename: format!("{stem}.md"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{export_file_stem, history_text, validate_thread_id};
    use crate::domain::chat::ChatAttachment;

    #[test]
    fn attached_files_are_written_out_for_the_model() {
        let files = vec![
            ChatAttachment {
                name: "a \"b\".md".into(),
                kind: "text".into(),
                text: Some("hello".into()),
                data_url: None,
                chars: 5,
                truncated: true,
                error: None,
            },
            ChatAttachment {
                name: "pic.png".into(),
                kind: "image".into(),
                text: None,
                data_url: Some("data:image/png;base64,AA".into()),
                chars: 0,
                truncated: false,
                error: None,
            },
        ];
        let text = history_text("Summarize", &files);
        assert!(
            text.starts_with("Summarize\n\n<attached_file name=\"a 'b'.md\" shortened=\"yes\">")
        );
        assert!(text.contains("hello\n</attached_file>"));
        assert!(!text.contains("pic.png"));
        assert_eq!(history_text("x", &[]), "x");
    }

    #[test]
    fn thread_ids_must_be_plain_tokens() {
        assert!(validate_thread_id("t_1731542400000").is_ok());
        assert!(validate_thread_id("a-b_c").is_ok());
        for bad in ["", "..", "../config", "..\\config", "a.b", "a b", "C:x"] {
            assert!(validate_thread_id(bad).is_err(), "{bad:?} must be rejected");
        }
        assert!(validate_thread_id(&"a".repeat(65)).is_err());
    }

    #[test]
    fn export_file_names_are_safe_everywhere() {
        assert_eq!(export_file_stem("Plan my week"), "Plan_my_week");
        assert_eq!(export_file_stem("a/b\\c:d?*\"<>|"), "a_b_c_d");
        assert_eq!(export_file_stem("..."), "chat");
        assert_eq!(export_file_stem(""), "chat");
        assert_eq!(export_file_stem(&"x".repeat(200)).len(), 60);
    }
}
