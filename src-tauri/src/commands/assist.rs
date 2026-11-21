//! Small helpers around the chat: improving a prompt, suggesting the next
//! one, learned starter prompts, undoing a learned skill, opening the files
//! a chat made, and the interface scale.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::app::paths::{app_dir, conversations_dir};
use crate::app::state::AppState;
use crate::domain::chat::ChatTargetRequest;
use crate::services::agent::oneshot;

fn target_for(
    state: &AppState,
    target: Option<&ChatTargetRequest>,
) -> Result<crate::services::inference::router::ChatTarget, AppError> {
    let cfg = {
        let store = state
            .config_store
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        store.load().map_err(AppError::internal)?
    };
    Ok(crate::commands::chat::resolve_target(state, &cfg, target)?.0)
}

const ENHANCE_PROMPT: &str = "You rewrite a user's message to an AI assistant so it gets a better answer. Keep the user's intent, language and facts. Make it clear and specific: the goal, the needed context, and the form of the answer. Add nothing the user did not imply, and do not answer it. Reply with the rewritten message only, no preface and no quotes.";

/// Rewrite the draft in the chat box into a clearer prompt.
#[tauri::command]
pub async fn chat_enhance_prompt(
    state: State<'_, AppState>,
    text: String,
    target: Option<ChatTargetRequest>,
) -> Result<String, AppError> {
    let draft: String = text.trim().chars().take(8000).collect();
    if draft.is_empty() {
        return Err(AppError::invalid_input("Write something first."));
    }
    let owned: AppState = (*state).clone();
    run_blocking(move || {
        let target = target_for(&owned, target.as_ref())?;
        let better = oneshot::complete(&owned, &target, ENHANCE_PROMPT, &draft, 600)?;
        if better.is_empty() {
            return Err(AppError::internal("The model gave no prompt back."));
        }
        Ok(better)
    })
    .await
}

const NEXT_PROMPT: &str = "Suggest the 3 most useful next messages the user could send in this conversation, written as the user would write it, in their language, each with a different idea. At most 12 words each. Reply with exactly 3 lines, one message per line, no numbering.";

/// Up to three suggested next messages, from the last question and answer.
#[tauri::command]
pub async fn chat_suggest_next(
    state: State<'_, AppState>,
    question: String,
    answer: String,
    target: Option<ChatTargetRequest>,
) -> Result<Vec<String>, AppError> {
    let q: String = question.chars().take(1500).collect();
    let tail: String = {
        let chars: Vec<char> = answer.chars().collect();
        chars[chars.len().saturating_sub(2500)..].iter().collect()
    };
    if q.trim().is_empty() && tail.trim().is_empty() {
        return Ok(Vec::new());
    }
    let owned: AppState = (*state).clone();
    run_blocking(move || {
        let target = target_for(&owned, target.as_ref())?;
        let text = format!("The user asked:\n{q}\n\nThe assistant answered (end of it):\n{tail}");
        let next = oneshot::complete(&owned, &target, NEXT_PROMPT, &text, 160)?;
        let nexts: Vec<String> = next
            .lines()
            .map(|line| {
                let mut line = line.trim();
                // The model may number its lines despite being told not to.
                for prefix in ["- ", "* ", "1. ", "2. ", "3. ", "1) ", "2) ", "3) "] {
                    if let Some(rest) = line.strip_prefix(prefix) {
                        line = rest;
                        break;
                    }
                }
                line.trim().to_string()
            })
            .filter(|line| (3..=160).contains(&line.chars().count()))
            .take(3)
            .collect();
        Ok(nexts)
    })
    .await
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Starter {
    pub title: String,
    pub prompt: String,
    /// How many saved chats began like this.
    pub uses: usize,
}

#[derive(Deserialize)]
struct SavedFirst {
    #[serde(default)]
    updated_at: u64,
    #[serde(default)]
    messages: Vec<SavedMessage>,
}

#[derive(Deserialize)]
struct SavedMessage {
    role: String,
    content: String,
}

/// Starter prompts learned from the user's own saved chats: the ways their
/// chats most often begin, newest first. Read on this computer only.
#[tauri::command]
pub async fn chat_starters() -> Result<Vec<Starter>, AppError> {
    run_blocking(|| Ok(learned_starters(&conversations_dir(), 4))).await
}

fn normalized(text: &str) -> String {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_of(prompt: &str) -> String {
    let words: Vec<&str> = prompt.split_whitespace().take(4).collect();
    let mut title = words.join(" ");
    title = title
        .trim_end_matches(|c: char| c.is_ascii_punctuation())
        .to_string();
    let mut chars = title.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => title,
    }
}

pub(crate) fn learned_starters(dir: &Path, limit: usize) -> Vec<Starter> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    // key -> (uses, newest time, prompt)
    let mut seen: std::collections::HashMap<String, (usize, u64, String)> =
        std::collections::HashMap::new();
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    files.sort();
    for path in files.into_iter().rev().take(300) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(saved) = serde_json::from_str::<SavedFirst>(&text) else {
            continue;
        };
        let Some(first) = saved.messages.iter().find(|m| m.role == "user") else {
            continue;
        };
        let prompt = first.content.trim();
        let words = prompt.split_whitespace().count();
        // Very short or very long openings make poor starters.
        if !(3..=40).contains(&words) || prompt.chars().count() > 220 || prompt.contains("```") {
            continue;
        }
        let key = normalized(prompt);
        let entry = seen
            .entry(key)
            .or_insert((0, saved.updated_at, prompt.to_string()));
        entry.0 += 1;
        if saved.updated_at > entry.1 {
            entry.1 = saved.updated_at;
            entry.2 = prompt.to_string();
        }
    }
    let mut ranked: Vec<(usize, u64, String)> = seen.into_values().collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    ranked
        .into_iter()
        .take(limit)
        .map(|(uses, _, prompt)| Starter {
            title: title_of(&prompt),
            prompt,
            uses,
        })
        .collect()
}

/// Undo a skill the model saved or improved on its own.
#[tauri::command]
pub fn skills_undo(state: State<'_, AppState>, change_id: String) -> Result<String, AppError> {
    state.skills.undo(&change_id)
}

/// A file a chat made: its path inside the chat's folder (the shared
/// workspace when `folder` is empty), checked against that folder.
fn chat_file(state: &AppState, folder: Option<&str>, path: &str) -> Result<PathBuf, AppError> {
    let jail = crate::commands::chat::folder_jail(state, folder)?;
    let trimmed = path.trim();
    let relative = trimmed
        .strip_prefix("workspace/")
        .filter(|_| folder.is_none_or(|f| f.trim().is_empty()))
        .unwrap_or(trimmed);
    let real = jail.resolve_path(if relative.is_empty() { "." } else { relative })?;
    if !real.exists() {
        return Err(AppError::invalid_input(format!(
            "{trimmed} is not there any more."
        )));
    }
    Ok(real)
}

/// Programs and scripts are never opened with their default app, which
/// would run them.
fn runs_when_opened(path: &Path) -> bool {
    const RUNNABLE: &[&str] = &[
        "exe",
        "bat",
        "cmd",
        "com",
        "msi",
        "msix",
        "appx",
        "ps1",
        "psm1",
        "vbs",
        "vbe",
        "js",
        "jse",
        "wsf",
        "wsh",
        "scr",
        "lnk",
        "url",
        "reg",
        "hta",
        "cpl",
        "pif",
        "sh",
        "bash",
        "zsh",
        "fish",
        "command",
        "tool",
        "app",
        "pkg",
        "dmg",
        "jar",
        "py",
        "pyw",
        "pl",
        "rb",
        "appimage",
        "desktop",
        "run",
        "bin",
        "workflow",
        "scpt",
        "applescript",
        "inf",
    ];
    if path.is_dir() {
        return path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("app"));
    }
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| RUNNABLE.iter().any(|r| e.eq_ignore_ascii_case(r)))
}

/// Open a file the chat made in its default app.
#[tauri::command]
pub fn files_open(
    state: State<'_, AppState>,
    path: String,
    folder: Option<String>,
) -> Result<(), AppError> {
    let real = chat_file(&state, folder.as_deref(), &path)?;
    if runs_when_opened(&real) {
        return Err(AppError::invalid_input(
            "This file is a program or script, so LocalAide does not run it. Open it with an editor, or show it in its folder.",
        ));
    }
    tauri_plugin_opener::open_path(&real, None::<&str>)
        .map_err(|e| AppError::internal(format!("The file could not be opened: {e}")))
}

/// Show a file (or the chat's folder, for an empty path) in Finder,
/// File Explorer or the file manager.
#[tauri::command]
pub fn files_reveal(
    state: State<'_, AppState>,
    path: String,
    folder: Option<String>,
) -> Result<(), AppError> {
    let real = chat_file(&state, folder.as_deref(), &path)?;
    let result = if path.trim().is_empty() {
        tauri_plugin_opener::open_path(&real, None::<&str>)
    } else {
        tauri_plugin_opener::reveal_item_in_dir(&real)
    };
    result.map_err(|e| AppError::internal(format!("The folder could not be shown: {e}")))
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenWithApp {
    pub id: String,
    pub name: String,
}

/// Editors found on this computer, for "Open with".
fn editors() -> Vec<(String, String, PathBuf)> {
    let mut out = Vec::new();
    #[cfg(target_os = "macos")]
    {
        for (id, name, app) in [
            ("vscode", "Visual Studio Code", "Visual Studio Code.app"),
            ("cursor", "Cursor", "Cursor.app"),
            ("zed", "Zed", "Zed.app"),
            ("sublime", "Sublime Text", "Sublime Text.app"),
            ("bbedit", "BBEdit", "BBEdit.app"),
            ("xcode", "Xcode", "Xcode.app"),
            ("textedit", "TextEdit", "TextEdit.app"),
            ("preview", "Preview", "Preview.app"),
        ] {
            for root in ["/Applications", "/System/Applications"] {
                let p = Path::new(root).join(app);
                if p.exists() {
                    out.push((id.to_string(), name.to_string(), p));
                    break;
                }
            }
        }
    }
    #[cfg(windows)]
    {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let programs = std::env::var_os("ProgramFiles").map(PathBuf::from);
        let candidates: Vec<(&str, &str, Option<PathBuf>)> = vec![
            (
                "vscode",
                "Visual Studio Code",
                local
                    .as_ref()
                    .map(|l| l.join("Programs/Microsoft VS Code/Code.exe")),
            ),
            (
                "vscode",
                "Visual Studio Code",
                programs
                    .as_ref()
                    .map(|p| p.join("Microsoft VS Code/Code.exe")),
            ),
            (
                "cursor",
                "Cursor",
                local.as_ref().map(|l| l.join("Programs/cursor/Cursor.exe")),
            ),
            (
                "notepadpp",
                "Notepad++",
                programs.as_ref().map(|p| p.join("Notepad++/notepad++.exe")),
            ),
            (
                "notepad",
                "Notepad",
                std::env::var_os("SystemRoot")
                    .map(|r| PathBuf::from(r).join("System32/notepad.exe")),
            ),
        ];
        for (id, name, path) in candidates {
            if let Some(p) = path
                && p.is_file()
                && !out.iter().any(|(i, _, _)| i == id)
            {
                out.push((id.to_string(), name.to_string(), p));
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(path) = std::env::var_os("PATH") {
            for (id, name, bin) in [
                ("vscode", "Visual Studio Code", "code"),
                ("zed", "Zed", "zed"),
                ("gnome-text-editor", "Text Editor", "gnome-text-editor"),
                ("gedit", "gedit", "gedit"),
                ("kate", "Kate", "kate"),
                ("mousepad", "Mousepad", "mousepad"),
                ("xed", "Xed", "xed"),
            ] {
                if let Some(p) = std::env::split_paths(&path)
                    .map(|d| d.join(bin))
                    .find(|p| p.is_file())
                {
                    out.push((id.to_string(), name.to_string(), p));
                }
            }
        }
    }
    out
}

/// The apps "Open with" offers on this computer. "other" opens the
/// system's own app chooser (Windows) or a file picker for an app (macOS).
#[tauri::command]
pub fn files_open_with_apps() -> Vec<OpenWithApp> {
    let mut apps: Vec<OpenWithApp> = editors()
        .into_iter()
        .map(|(id, name, _)| OpenWithApp { id, name })
        .collect();
    if cfg!(any(windows, target_os = "macos")) {
        apps.push(OpenWithApp {
            id: "other".into(),
            name: "Another app…".into(),
        });
    }
    apps
}

/// Open a file the chat made with the app the user picked.
#[tauri::command]
pub async fn files_open_with(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    folder: Option<String>,
    app_id: String,
) -> Result<(), AppError> {
    let real = chat_file(&state, folder.as_deref(), &path)?;
    if app_id == "other" {
        return open_with_chooser(&app, &real).await;
    }
    let (_, _, program) = editors()
        .into_iter()
        .find(|(id, _, _)| *id == app_id)
        .ok_or_else(|| AppError::invalid_input("That app is not on this computer."))?;
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-a").arg(&program);
        c
    } else {
        std::process::Command::new(&program)
    };
    cmd.arg(&real);
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| AppError::internal(format!("The app did not start: {e}")))
}

#[cfg(windows)]
async fn open_with_chooser(_app: &AppHandle, file: &Path) -> Result<(), AppError> {
    // The system's "How do you want to open this file?" window.
    let system = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    std::process::Command::new(system.join("System32").join("rundll32.exe"))
        .arg("shell32.dll,OpenAs_RunDLL")
        .arg(file)
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::internal(format!("The app chooser did not open: {e}")))
}

#[cfg(target_os = "macos")]
async fn open_with_chooser(app: &AppHandle, file: &Path) -> Result<(), AppError> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Open with")
        .set_directory("/Applications")
        .add_filter("Apps", &["app"])
        .pick_file(move |picked| {
            let _ = tx.send(picked);
        });
    let Some(picked) = rx.await.ok().flatten() else {
        return Ok(());
    };
    let picked = picked
        .into_path()
        .map_err(|e| AppError::internal(e.to_string()))?;
    std::process::Command::new("open")
        .arg("-a")
        .arg(picked)
        .arg(file)
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::internal(format!("The app did not start: {e}")))
}

#[cfg(not(any(windows, target_os = "macos")))]
async fn open_with_chooser(_app: &AppHandle, _file: &Path) -> Result<(), AppError> {
    Err(AppError::invalid_input(
        "Pick one of the listed apps on Linux.",
    ))
}

/// The interface scale, 0.7 to 2.5 (70% to 250%).
fn scale_file() -> PathBuf {
    app_dir().join("ui-scale.json")
}

pub fn saved_scale() -> f64 {
    std::fs::read_to_string(scale_file())
        .ok()
        .and_then(|t| t.trim().parse::<f64>().ok())
        .filter(|s| s.is_finite())
        .map(|s| s.clamp(0.7, 2.5))
        .unwrap_or(1.0)
}

#[tauri::command]
pub fn ui_scale_get() -> f64 {
    saved_scale()
}

/// Zoom the whole window and remember it for the next start.
#[tauri::command]
pub fn ui_scale_set(app: AppHandle, scale: f64) -> Result<f64, AppError> {
    if !scale.is_finite() {
        return Err(AppError::invalid_input("The scale must be a number."));
    }
    let scale = (scale.clamp(0.7, 2.5) * 100.0).round() / 100.0;
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_zoom(scale)
            .map_err(|e| AppError::internal(format!("The window could not be scaled: {e}")))?;
    }
    std::fs::write(scale_file(), scale.to_string())
        .map_err(|e| AppError::storage(format!("The scale could not be saved: {e}")))?;
    Ok(scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starters_come_from_how_chats_begin() {
        let dir = std::env::temp_dir().join(format!("la-starters-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let chat = |name: &str, at: u64, first: &str| {
            let json = serde_json::json!({
                "id": name, "title": first, "created_at": at, "updated_at": at,
                "messages": [{"role": "user", "content": first}, {"role": "assistant", "content": "ok"}]
            });
            std::fs::write(dir.join(format!("{name}.json")), json.to_string()).unwrap();
        };
        chat("a", 1, "Review this pull request for bugs");
        chat("b", 2, "review this pull request for bugs!");
        chat("c", 3, "Translate this email to Arabic please");
        chat("d", 4, "hi");
        let got = learned_starters(&dir, 4);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].uses, 2);
        assert_eq!(got[0].title, "Review this pull request");
        assert_eq!(got[1].prompt, "Translate this email to Arabic please");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scripts_are_not_opened_by_default() {
        assert!(runs_when_opened(Path::new("setup.EXE")));
        assert!(runs_when_opened(Path::new("run.sh")));
        assert!(!runs_when_opened(Path::new("report.md")));
        assert!(!runs_when_opened(Path::new("index.html")));
    }
}
