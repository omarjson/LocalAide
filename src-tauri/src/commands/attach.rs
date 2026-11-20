//! Files attached to a chat message: picked with the system file picker or
//! dropped on the window. Text, code and PDF files are read as text for the
//! model; images become data URLs for vision models.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::domain::chat::ChatAttachment;
use crate::services::library::ingest::{FileIngester, decode_text};

const MAX_FILES: usize = 8;
const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
/// About 20,000 tokens per file; longer files are shortened.
const MAX_TEXT_CHARS: usize = 80_000;
/// A dropped path stays readable this long.
const DROP_TTL: Duration = Duration::from_secs(120);

const IMAGE_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("webp", "image/webp"),
    ("gif", "image/gif"),
];

/// Standard base64, for image data URLs.
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// True when the bytes look like a binary file rather than text.
fn looks_binary(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8192)];
    // UTF-16 text has many zero bytes but starts with a byte order mark.
    if head.starts_with(&[0xFF, 0xFE]) || head.starts_with(&[0xFE, 0xFF]) {
        return false;
    }
    head.contains(&0)
}

fn failed(name: String, message: &str) -> ChatAttachment {
    ChatAttachment {
        name,
        kind: "text".into(),
        text: None,
        data_url: None,
        chars: 0,
        truncated: false,
        error: Some(message.to_string()),
    }
}

/// Read one file for chat.
pub fn read_attachment(path: &Path) -> ChatAttachment {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let Ok(meta) = std::fs::metadata(path) else {
        return failed(name, "The file could not be opened.");
    };
    if !meta.is_file() {
        return failed(
            name,
            "Folders cannot be attached. Add them to the library on the Search screen.",
        );
    }
    if let Some((_, mime)) = IMAGE_TYPES.iter().find(|(e, _)| *e == ext) {
        if meta.len() > MAX_IMAGE_BYTES {
            return failed(name, "The image is larger than 10 MB.");
        }
        return match std::fs::read(path) {
            Ok(bytes) => ChatAttachment {
                name,
                kind: "image".into(),
                text: None,
                data_url: Some(format!("data:{mime};base64,{}", base64(&bytes))),
                chars: 0,
                truncated: false,
                error: None,
            },
            Err(_) => failed(name, "The image could not be read."),
        };
    }
    if meta.len() > MAX_FILE_BYTES {
        return failed(name, "The file is larger than 25 MB.");
    }
    let Ok(bytes) = std::fs::read(path) else {
        return failed(name, "The file could not be read.");
    };
    let text = if ext == "pdf" {
        match FileIngester::extract_pdf_text(&bytes) {
            Ok(t) if !t.trim().is_empty() => t,
            _ => {
                return failed(
                    name,
                    "No text could be read from this PDF. It may be a scan.",
                );
            }
        }
    } else if looks_binary(&bytes) {
        return failed(
            name,
            "This file type cannot be read as text. Attach text, code, Markdown, CSV, JSON or PDF files.",
        );
    } else {
        decode_text(&bytes)
    };
    let total = text.chars().count();
    let truncated = total > MAX_TEXT_CHARS;
    let text: String = if truncated {
        text.chars().take(MAX_TEXT_CHARS).collect()
    } else {
        text
    };
    ChatAttachment {
        name,
        kind: "text".into(),
        chars: text.chars().count(),
        text: Some(text),
        data_url: None,
        truncated,
        error: None,
    }
}

/// Pick files with the system file picker and read them.
#[tauri::command]
pub async fn chat_attach_pick(app: AppHandle) -> Result<Vec<ChatAttachment>, AppError> {
    run_blocking(move || {
        let Some(picked) = app
            .dialog()
            .file()
            .set_title("Attach files")
            .blocking_pick_files()
        else {
            return Ok(Vec::new());
        };
        let paths: Vec<PathBuf> = picked
            .into_iter()
            .filter_map(|p| p.into_path().ok())
            .take(MAX_FILES)
            .collect();
        Ok(paths.iter().map(|p| read_attachment(p)).collect())
    })
    .await
}

/// Read files the user just dropped on the window. Only paths from a real
/// drop are read.
#[tauri::command]
pub async fn chat_attach_dropped(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<ChatAttachment>, AppError> {
    let allowed: Vec<PathBuf> = {
        let mut dropped = state
            .dropped_paths
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        dropped.retain(|(_, at)| at.elapsed() < DROP_TTL);
        let wanted: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let allowed: Vec<PathBuf> = wanted
            .into_iter()
            .filter(|w| dropped.iter().any(|(p, _)| p == w))
            .take(MAX_FILES)
            .collect();
        dropped.retain(|(p, _)| !allowed.contains(p));
        allowed
    };
    if allowed.is_empty() && !paths.is_empty() {
        return Err(AppError::invalid_input(
            "Drop the files on the window again, or use the attach button.",
        ));
    }
    run_blocking(move || Ok(allowed.iter().map(|p| read_attachment(p)).collect())).await
}

/// Remember dropped paths for `chat_attach_dropped`.
pub fn remember_drop(state: &AppState, paths: &[PathBuf]) {
    let mut dropped = state
        .dropped_paths
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    dropped.retain(|(_, at)| at.elapsed() < DROP_TTL);
    let now = Instant::now();
    for p in paths.iter().take(64) {
        dropped.push((p.clone(), now));
    }
    let extra = dropped.len().saturating_sub(64);
    dropped.drain(..extra);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn text_files_are_read_and_binaries_refused() {
        let dir = std::env::temp_dir().join(format!("la-attach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = dir.join("notes.md");
        std::fs::write(&text, "# Hello\nworld").unwrap();
        let a = read_attachment(&text);
        assert_eq!(a.kind, "text");
        assert_eq!(a.text.as_deref(), Some("# Hello\nworld"));
        assert!(a.error.is_none());

        let bin = dir.join("tool.bin");
        std::fs::write(&bin, [0u8, 1, 2, 0, 5]).unwrap();
        assert!(read_attachment(&bin).error.is_some());

        let img = dir.join("dot.png");
        std::fs::write(&img, [137u8, 80, 78, 71]).unwrap();
        let a = read_attachment(&img);
        assert_eq!(a.kind, "image");
        assert!(a.data_url.unwrap().starts_with("data:image/png;base64,"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
