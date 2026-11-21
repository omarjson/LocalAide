//! Pages the model draws in the chat (```html mockups). Each page is kept
//! in memory and served from the `mockup:` scheme with its own strict
//! Content-Security-Policy: no network, no access to the app or the user's
//! files, inline scripts and styles only. The chat shows it in an iframe
//! sandboxed without same-origin, so it cannot reach the app's IPC either.

use std::collections::VecDeque;
use std::sync::Mutex;

use tauri::http::{Request, Response, StatusCode, header};

use crate::app::error::AppError;

const MAX_PAGES: usize = 64;
const MAX_PAGE_BYTES: usize = 512 * 1024;

/// What a page may do. connect-src is not listed, so it falls back to
/// 'none': no fetch, no WebSocket.
pub const FRAME_CSP: &str = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; form-action 'none'; base-uri 'none'; frame-src 'none'; worker-src 'none'";

static PAGES: Mutex<VecDeque<(String, String)>> = Mutex::new(VecDeque::new());

/// Script added to every page: `localaide.choose(text)` sends the user's
/// pick to the chat, and the page reports its height so the frame fits.
const BRIDGE: &str = r#"<script>
(function(){
  var sent = false;
  window.localaide = { choose: function(text){
    if (sent) return false;
    text = String(text == null ? "" : text).slice(0, 500);
    if (!text.trim()) return false;
    sent = true;
    parent.postMessage({ localaide: "choose", text: text }, "*");
    return true;
  }};
  function size(){ parent.postMessage({ localaide: "size", height: document.documentElement.scrollHeight }, "*"); }
  addEventListener("load", size);
  new ResizeObserver(size).observe(document.documentElement);
})();
</script>"#;

/// Keep a page and return its id.
pub fn put(html: &str) -> Result<String, AppError> {
    if html.len() > MAX_PAGE_BYTES {
        return Err(AppError::invalid_input(
            "This page is too big to show (over 512 KB).",
        ));
    }
    let mut bytes = [0u8; 12];
    getrandom::fill(&mut bytes).map_err(|_| AppError::internal("No random source."))?;
    let id = hex::encode(bytes);
    let doc = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><style>html,body{{margin:0;font-family:system-ui,-apple-system,'Segoe UI',sans-serif}}</style>{BRIDGE}</head><body>{html}</body></html>"
    );
    let mut pages = PAGES.lock().unwrap_or_else(|e| e.into_inner());
    pages.push_back((id.clone(), doc));
    while pages.len() > MAX_PAGES {
        pages.pop_front();
    }
    Ok(id)
}

/// Serve `mockup://localhost/<id>` (`http://mockup.localhost/<id>` on
/// Windows).
pub fn serve(request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let id = request.uri().path().trim_start_matches('/').to_string();
    let found = PAGES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| v.clone());
    let builder = Response::builder()
        .header(header::CONTENT_SECURITY_POLICY, FRAME_CSP)
        .header("X-Content-Type-Options", "nosniff")
        .header("Referrer-Policy", "no-referrer")
        .header(header::CACHE_CONTROL, "no-store");
    match found {
        Some(doc) => builder
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(doc.into_bytes()),
        None => builder
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(b"This page is gone. Ask again to draw it.".to_vec()),
    }
    .unwrap_or_else(|_| Response::new(Vec::new()))
}

#[tauri::command]
pub fn render_frame_put(html: String) -> Result<String, AppError> {
    put(&html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_served_with_a_strict_policy() {
        let id = put("<button onclick=\"localaide.choose('A')\">A</button>").unwrap();
        let req = Request::builder()
            .uri(format!("mockup://localhost/{id}"))
            .body(Vec::new())
            .unwrap();
        let res = serve(&req);
        assert_eq!(res.status(), StatusCode::OK);
        let csp = res.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap();
        assert!(csp.starts_with("default-src 'none'"));
        assert!(!csp.contains("connect-src"), "no network");
        let body = String::from_utf8(res.body().clone()).unwrap();
        assert!(body.contains("localaide.choose('A')"));
        assert!(body.contains("window.localaide"));

        let missing = Request::builder()
            .uri("mockup://localhost/nope")
            .body(Vec::new())
            .unwrap();
        assert_eq!(serve(&missing).status(), StatusCode::NOT_FOUND);
        assert!(put(&"x".repeat(MAX_PAGE_BYTES + 1)).is_err());
    }
}
