//! Native file pickers, and opening web links in the user's browser.

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::services::library::ingest::SUPPORTED_EXTENSIONS;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

/// Shows the system folder picker. Returns `None` when the user closes it.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, AppError> {
    // The picker blocks until the user answers, so it runs on a worker thread.
    run_blocking(move || {
        let picked = app
            .dialog()
            .file()
            .set_title("Choose a folder")
            .blocking_pick_folder();
        picked_path(picked)
    })
    .await
}

/// Shows the system file picker, limited to file types the library can read.
/// Returns `None` when the user closes it.
#[tauri::command]
pub async fn pick_document(app: AppHandle) -> Result<Option<String>, AppError> {
    run_blocking(move || {
        let picked = app
            .dialog()
            .file()
            .set_title("Choose a document")
            .add_filter("Documents", SUPPORTED_EXTENSIONS)
            .blocking_pick_file();
        picked_path(picked)
    })
    .await
}

/// Opens a web link in the user's browser. Only http and https links open,
/// so a link in an answer or a web page cannot start a program or open a
/// file on this computer.
#[tauri::command]
pub async fn open_link(url: String) -> Result<(), AppError> {
    let link = web_link(&url)?;
    run_blocking(move || {
        tauri_plugin_opener::open_url(link.as_str(), None::<&str>)
            .map_err(|e| AppError::internal(format!("The link could not open: {e}")))
    })
    .await
}

fn picked_path(picked: Option<FilePath>) -> Result<Option<String>, AppError> {
    match picked {
        None => Ok(None),
        Some(path) => path
            .into_path()
            .map(|p| Some(p.to_string_lossy().into_owned()))
            .map_err(|e| AppError::invalid_input(format!("That location cannot be used: {e}"))),
    }
}

/// Accepts only http(s) addresses with a host and no user name or password.
fn web_link(url: &str) -> Result<url::Url, AppError> {
    let parsed = url::Url::parse(url.trim())
        .map_err(|_| AppError::invalid_input("That link is not a web address."))?;
    let web = matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some();
    if !web || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::invalid_input("Only web links can open."));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::web_link;

    #[test]
    fn only_plain_web_links_open() {
        for good in ["https://example.com", "http://example.com/a?b=1#c"] {
            assert!(web_link(good).is_ok(), "{good} should open");
        }
        for bad in [
            "file:///C:/Windows/system32/calc.exe",
            "javascript:alert(1)",
            "ms-msdt:/id PCWDiagnostic",
            "mailto:someone@example.com",
            "https://user:secret@example.com",
            "not a link",
        ] {
            assert!(web_link(bad).is_err(), "{bad} must not open");
        }
    }
}
