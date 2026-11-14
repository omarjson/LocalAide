//! Web search and page reading over public HTTP APIs. Providers are either
//! keyless (DuckDuckGo HTML results, Wikipedia, the Jina Reader) or take a
//! key the user pastes into Settings (Exa, Firecrawl). Keys live in
//! config.json on this machine only, travel only to their own provider, and
//! are excluded from backups unless explicitly included. Every request is
//! an explicit user action — nothing here runs in the background.
//!
//! The base URLs are constructor parameters so tests can point the engine
//! at loopback servers; parsing lives in pure functions tested without any
//! network.

use crate::app::error::AppError;
use serde::Serialize;
use std::io::Read as _;
use std::time::Duration;

/// Cap on any single response body, so a broken or hostile endpoint cannot
/// stream unbounded data into memory.
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// One selectable provider, as the Settings screen renders it.
#[derive(Debug, Clone, Serialize)]
pub struct WebProvider {
    pub id: &'static str,
    pub label: &'static str,
    pub needs_key: bool,
    /// "search", "crawl", or "search+crawl".
    pub supports: &'static str,
}

pub const PROVIDER_TABLE: &[WebProvider] = &[
    WebProvider {
        id: "duckduckgo",
        label: "DuckDuckGo",
        needs_key: false,
        supports: "search",
    },
    WebProvider {
        id: "wikipedia",
        label: "Wikipedia",
        needs_key: false,
        supports: "search",
    },
    WebProvider {
        id: "jina",
        label: "Jina Reader",
        needs_key: false,
        supports: "crawl",
    },
    WebProvider {
        id: "exa",
        label: "Exa",
        needs_key: true,
        supports: "search",
    },
    WebProvider {
        id: "firecrawl",
        label: "Firecrawl",
        needs_key: true,
        supports: "search+crawl",
    },
];

/// Provider ids accepted by the engine, in one place for validation.
fn known_provider(id: &str) -> bool {
    PROVIDER_TABLE.iter().any(|p| p.id == id)
}

/// The keys the user configured, read from config per request. Keyless
/// providers ignore this; keyed providers fail with `WEB_NO_KEY` when their
/// slot is empty.
#[derive(Debug, Clone, Default)]
pub struct WebSearchKeys {
    pub exa_api_key: Option<String>,
    pub firecrawl_api_key: Option<String>,
}

impl WebSearchKeys {
    /// The stored key for a provider, or `WEB_NO_KEY` when it is missing or
    /// blank. Keyless providers return an empty string.
    pub(crate) fn for_provider(&self, id: &str) -> Result<&str, AppError> {
        let key = match id {
            "exa" => self.exa_api_key.as_deref(),
            "firecrawl" => self.firecrawl_api_key.as_deref(),
            _ => return Ok(""),
        };
        key.filter(|k| !k.trim().is_empty())
            .ok_or_else(|| {
                AppError::new(
                    "WEB_NO_KEY",
                    format!(
                        "{id} needs an API key — add one in Settings → Web search. DuckDuckGo and Wikipedia need no key."
                    ),
                )
            })
    }
}

/// Base URLs per provider. `Bases::production()` is what the app uses;
/// tests construct their own pointing at loopback servers.
#[derive(Debug, Clone)]
pub struct Bases {
    pub ddg: String,
    pub wikipedia: String,
    pub jina: String,
    pub exa: String,
    pub firecrawl: String,
}

impl Bases {
    pub fn production() -> Self {
        Self {
            // The lite endpoint serves a small regular table of results and
            // is far cheaper to parse than the full HTML page.
            ddg: "https://lite.duckduckgo.com".into(),
            wikipedia: "https://en.wikipedia.org".into(),
            jina: "https://r.jina.ai".into(),
            exa: "https://api.exa.ai".into(),
            firecrawl: "https://api.firecrawl.dev".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WebHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebPage {
    pub title: Option<String>,
    pub url: String,
    pub content: String,
}

pub struct WebSearchEngine {
    client: reqwest::blocking::Client,
    bases: Bases,
}

impl Default for WebSearchEngine {
    fn default() -> Self {
        Self::with_bases(Bases::production())
    }
}

impl WebSearchEngine {
    pub fn new() -> Self {
        Self::with_bases(Bases::production())
    }

    pub fn with_bases(bases: Bases) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self { client, bases }
    }

    /// Search the web with the chosen provider.
    pub fn search(
        &self,
        provider: &str,
        query: &str,
        limit: u32,
        keys: &WebSearchKeys,
    ) -> Result<Vec<WebHit>, AppError> {
        if !known_provider(provider) {
            return Err(AppError::new(
                "WEB_UNKNOWN_PROVIDER",
                format!("'{provider}' is not a web search provider this build knows."),
            ));
        }
        let limit = limit.clamp(1, 20) as usize;
        match provider {
            "duckduckgo" => {
                let body = self.post_form(&format!("{}/lite/", self.bases.ddg), &[("q", query)])?;
                parse_ddg_html(&body, limit)
            }
            "wikipedia" => {
                let url = format!(
                    "{}/w/rest.php/v1/search/page?q={}&limit={limit}",
                    self.bases.wikipedia,
                    urlencode(query)
                );
                let body = self.get_capped(&url)?;
                parse_wikipedia_json(&body, limit, &self.bases.wikipedia)
            }
            "exa" => {
                let key = keys.for_provider("exa")?;
                let body = serde_json::json!({
                    "query": query,
                    "numResults": limit,
                    "contents": {"text": {"maxCharacters": 500}}
                })
                .to_string();
                let response = self.post_json(
                    &format!("{}/search", self.bases.exa),
                    &[("x-api-key", key)],
                    &body,
                )?;
                parse_exa_json(&response, limit)
            }
            "firecrawl" => {
                let key = keys.for_provider("firecrawl")?;
                let body = serde_json::json!({"query": query, "limit": limit}).to_string();
                let response = self.post_json(
                    &format!("{}/v2/search", self.bases.firecrawl),
                    &[("Authorization", &format!("Bearer {key}"))],
                    &body,
                )?;
                parse_firecrawl_search_json(&response, limit)
            }
            // The match above is total over PROVIDER_TABLE; crawl-only
            // providers cannot serve searches.
            _ => Err(AppError::new(
                "WEB_UNKNOWN_PROVIDER",
                format!("'{provider}' cannot search the web."),
            )),
        }
    }

    /// Read one page as text. Firecrawl renders markdown when its key is
    /// configured; the keyless default is the Jina Reader.
    pub fn read_page(
        &self,
        url: &str,
        provider: &str,
        keys: &WebSearchKeys,
    ) -> Result<WebPage, AppError> {
        validate_crawl_url(url)?;
        match provider {
            "firecrawl" => {
                let key = keys.for_provider("firecrawl")?;
                let body = serde_json::json!({
                    "url": url,
                    "formats": ["markdown"],
                    "onlyMainContent": true
                })
                .to_string();
                let response = self.post_json(
                    &format!("{}/v2/scrape", self.bases.firecrawl),
                    &[("Authorization", &format!("Bearer {key}"))],
                    &body,
                )?;
                parse_firecrawl_scrape_json(&response, url)
            }
            "jina" => {
                // JSON mode gives a clean title/content split; the keyless
                // tier is IP-throttled (~20 req/min).
                let (body, header_title) = self.get_capped_with_title_accept(
                    &format!("{}/{}", self.bases.jina, url),
                    "application/json",
                )?;
                parse_jina_response(&body, header_title.as_deref(), url)
            }
            other => Err(AppError::new(
                "WEB_UNKNOWN_PROVIDER",
                format!("'{other}' cannot read pages — use Firecrawl or Jina Reader."),
            )),
        }
    }

    /// GET with a response-size cap. Returns (body, X-Title header).
    fn get_capped_with_title_accept(
        &self,
        url: &str,
        accept: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", "LocalAide-web/0.1")
            .header("Accept", accept)
            .send()
            .map_err(|e| AppError::internal(format!("Web request failed: {e}")))?;
        let status = response.status();
        let title = response
            .headers()
            .get("x-title")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        check_status(status)?;
        let body = read_capped(response)?;
        Ok((body, title))
    }

    fn get_capped(&self, url: &str) -> Result<String, AppError> {
        self.get_capped_with_title_accept(url, "*/*")
            .map(|(body, _)| body)
    }

    fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Result<String, AppError> {
        let mut request = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "LocalAide-web/0.1");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request
            .body(body.to_string())
            .send()
            .map_err(|e| AppError::internal(format!("Web request failed: {e}")))?;
        check_status(response.status())?;
        read_capped(response)
    }

    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<String, AppError> {
        let response = self
            .client
            .post(url)
            .header("User-Agent", "LocalAide-web/0.1")
            .form(form)
            .send()
            .map_err(|e| AppError::internal(format!("Web request failed: {e}")))?;
        check_status(response.status())?;
        read_capped(response)
    }
}

fn check_status(status: reqwest::StatusCode) -> Result<(), AppError> {
    if status.as_u16() == 429 {
        return Err(AppError::new(
            "WEB_RATE_LIMITED",
            "The provider is rate-limiting this machine — wait a moment or switch provider in Settings.",
        ));
    }
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(AppError::new(
            "WEB_AUTH_FAILED",
            "The provider rejected the API key. Check it in Settings → Web search.",
        ));
    }
    if !status.is_success() {
        return Err(AppError::internal(format!(
            "The web provider returned HTTP {status}"
        )));
    }
    Ok(())
}

fn read_capped(mut response: reqwest::blocking::Response) -> Result<String, AppError> {
    let mut body = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let read = response
            .read(&mut chunk)
            .map_err(|e| AppError::internal(format!("Web response stream failed: {e}")))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(AppError::invalid_input(
                "The web response exceeds the size cap",
            ));
        }
    }
    String::from_utf8(body)
        .map_err(|_| AppError::invalid_input("The web response is not valid UTF-8"))
}

/// Very small percent-encoding for query strings — enough for search terms.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Inverse of `urlencode` for `uddg=` hrefs.
fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Decode the handful of HTML entities DDG snippets actually use.
fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// Drop `<...>` spans from a DDG snippet fragment.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Crawl targets must be plain http(s) URLs without embedded credentials.
pub fn validate_crawl_url(url: &str) -> Result<(), AppError> {
    let parsed = url::Url::parse(url)
        .map_err(|_| AppError::invalid_input("That is not a valid URL to read."))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::invalid_input(
            "Do not put a username or password in the URL.",
        ));
    }
    match parsed.scheme() {
        "https" | "http" => Ok(()),
        _ => Err(AppError::invalid_input(
            "Only http and https pages can be read.",
        )),
    }
}

/// Pull the `uddg=` target out of a DuckDuckGo redirect href, or pass the
/// href through when it is already a direct link.
fn resolve_ddg_href(href: &str) -> String {
    let href = decode_entities(href);
    let full = if let Some(rest) = href.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        href
    };
    if let Some(start) = full.find("uddg=") {
        let rest = &full[start + 5..];
        let encoded = rest.split('&').next().unwrap_or(rest);
        return urldecode(encoded);
    }
    full
}

/// Parse a DuckDuckGo results page (the lite table or the full HTML page).
/// Pure function so tests need no network. The current layout marks result
/// titles with `result-link` anchors and snippets with `result-snippet`
/// elements; the older `result__a`/`result__snippet` names are accepted as
/// a fallback so a cosmetic change on DDG's side degrades slowly instead
/// of breaking outright.
pub fn parse_ddg_html(body: &str, limit: usize) -> Result<Vec<WebHit>, AppError> {
    if body.contains("anomaly") || body.contains("unusual traffic") {
        return Err(AppError::new(
            "WEB_RATE_LIMITED",
            "DuckDuckGo is rate-limiting this machine — switch provider in Settings or try again later.",
        ));
    }
    let title_marker = if body.contains("class=\"result-link\"") {
        "class=\"result-link\""
    } else {
        "class=\"result__a\""
    };
    let snippet_marker = if body.contains("class=\"result-snippet\"") {
        "class=\"result-snippet\""
    } else {
        "class=\"result__snippet\""
    };
    let mut hits = Vec::new();
    let mut rest = body;
    while hits.len() < limit {
        let Some(marker) = rest.find(title_marker) else {
            break;
        };
        rest = &rest[marker..];
        // href="..." inside the anchor's open tag.
        let Some(href_start) = rest.find("href=\"") else {
            break;
        };
        let after_href = &rest[href_start + 6..];
        let Some(href_end) = after_href.find('"') else {
            break;
        };
        let href = &after_href[..href_end];
        // Title: text between the end of the open tag and </a>.
        let after_open = &after_href[href_end..];
        let Some(tag_end) = after_open.find('>') else {
            break;
        };
        let after_tag = &after_open[tag_end + 1..];
        let Some(close) = after_tag.find("</a>") else {
            break;
        };
        let title = decode_entities(&strip_tags(&after_tag[..close]))
            .trim()
            .to_string();

        // Snippet: the next snippet element, if it precedes the next result.
        let snippet_window = &after_open[close + 4..];
        let next_result = snippet_window
            .find(title_marker)
            .unwrap_or(snippet_window.len());
        let snippet = match snippet_window.find(snippet_marker) {
            Some(pos) if pos < next_result => {
                let after = &snippet_window[pos..];
                if let Some(gt) = after.find('>') {
                    let tail = &after[gt + 1..];
                    // Snippets live in a <td>, <div>, or <a> depending on
                    // the layout; take whichever closes first.
                    let end = ["</td>", "</div>", "</a>"]
                        .iter()
                        .filter_map(|c| tail.find(c))
                        .min()
                        .unwrap_or(tail.len());
                    decode_entities(&strip_tags(&tail[..end]))
                        .trim()
                        .to_string()
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };

        if !title.is_empty() {
            hits.push(WebHit {
                title,
                url: resolve_ddg_href(href),
                snippet,
            });
        }
        rest = snippet_window;
    }
    Ok(hits)
}

/// Parse a Jina Reader response. JSON mode (Accept: application/json)
/// gives {code, data:{title, url, content}}; plain text falls back to
/// being used as the content verbatim. `header_title` is the X-Title
/// response header, used when the JSON has no title.
pub fn parse_jina_response(
    body: &str,
    header_title: Option<&str>,
    url: &str,
) -> Result<WebPage, AppError> {
    if let Ok(root) = serde_json::from_str::<serde_json::Value>(body)
        && root.get("code").and_then(|v| v.as_i64()).is_some()
    {
        let content = root
            .pointer("/data/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = root
            .pointer("/data/title")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| {
                header_title
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            });
        return Ok(WebPage {
            title,
            url: url.to_string(),
            content,
        });
    }
    Ok(WebPage {
        title: header_title
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        url: url.to_string(),
        content: body.to_string(),
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
struct WikiEntry {
    title: String,
    /// Pre-slugified article key ("Rust_(programming_language)").
    #[serde(default)]
    key: String,
    #[serde(default)]
    excerpt: String,
}

/// Parse the Wikipedia REST search response into hits pointing at the
/// article pages. Pure function; `base` is injected for tests.
pub fn parse_wikipedia_json(body: &str, limit: usize, base: &str) -> Result<Vec<WebHit>, AppError> {
    let root: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| AppError::invalid_input(format!("Wikipedia response is invalid: {e}")))?;
    let entries: Vec<WikiEntry> = serde_json::from_value(
        root.get("pages")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|e| AppError::invalid_input(format!("Wikipedia response is invalid: {e}")))?;
    Ok(entries
        .into_iter()
        .take(limit)
        .map(|e| {
            let slug = if e.key.is_empty() {
                e.title.replace(' ', "_")
            } else {
                e.key
            };
            WebHit {
                url: format!("{}/wiki/{}", base.trim_end_matches('/'), slug),
                title: e.title,
                snippet: decode_entities(&strip_tags(&e.excerpt)),
            }
        })
        .collect())
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ExaEntry {
    #[serde(default)]
    title: String,
    url: String,
    #[serde(default)]
    text: String,
}

/// Parse an Exa `/search` response. Pure function.
pub fn parse_exa_json(body: &str, limit: usize) -> Result<Vec<WebHit>, AppError> {
    let root: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| AppError::invalid_input(format!("Exa response is invalid: {e}")))?;
    let entries: Vec<ExaEntry> = serde_json::from_value(
        root.get("results")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|e| AppError::invalid_input(format!("Exa response is invalid: {e}")))?;
    Ok(entries
        .into_iter()
        .take(limit)
        .map(|e| {
            let snippet: String = e.text.chars().take(400).collect();
            WebHit {
                title: if e.title.is_empty() {
                    e.url.clone()
                } else {
                    e.title
                },
                url: e.url,
                snippet,
            }
        })
        .collect())
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FcWebEntry {
    #[serde(default)]
    title: String,
    url: String,
    #[serde(default)]
    description: String,
}

/// Parse a Firecrawl `/v1/search` response. Pure function.
pub fn parse_firecrawl_search_json(body: &str, limit: usize) -> Result<Vec<WebHit>, AppError> {
    let root: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| AppError::invalid_input(format!("Firecrawl response is invalid: {e}")))?;
    let entries: Vec<FcWebEntry> = serde_json::from_value(
        root.pointer("/data/web")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|e| AppError::invalid_input(format!("Firecrawl response is invalid: {e}")))?;
    Ok(entries
        .into_iter()
        .take(limit)
        .map(|e| WebHit {
            title: if e.title.is_empty() {
                e.url.clone()
            } else {
                e.title
            },
            url: e.url,
            snippet: e.description,
        })
        .collect())
}

/// Parse a Firecrawl `/v1/scrape` response into a page. Pure function.
pub fn parse_firecrawl_scrape_json(body: &str, url: &str) -> Result<WebPage, AppError> {
    let root: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| AppError::invalid_input(format!("Firecrawl response is invalid: {e}")))?;
    let markdown = root
        .pointer("/data/markdown")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::invalid_input("Firecrawl returned no page content for that URL"))?
        .to_string();
    let title = root
        .pointer("/data/metadata/title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(WebPage {
        title,
        url: url.to_string(),
        content: markdown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The current (2026) DuckDuckGo markup: `result-link` anchors with
    /// `result-snippet` elements. Double-hash delimiters because the
    /// fixture contains `"#` inside `href="#"`.
    const DDG_FIXTURE: &str = r##"<html><body>
    <div class="result">
      <h2><a rel="nofollow" class="result-link" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F&amp;rut=abc">The Rust Programming Language</a></h2>
      <div class="result-snippet">A language empowering <b>everyone</b> to build reliable software &amp; more.</div>
    </div>
    <table>
      <tr><td><a rel="nofollow" class="result-link" href="https://doc.rust-lang.org/book/">The Rust Book</a></td></tr>
      <tr><td class="result-snippet">Learn <span class="searchmatch">Rust</span> from the official book.</td></tr>
    </table>
    </body></html>"##;

    /// The legacy layout some mirrors and caches still serve.
    const DDG_LEGACY_FIXTURE: &str = r##"<html><body>
    <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fold.example%2F&amp;rut=x">Old Result</a>
    <a class="result__snippet" href="#">Legacy <b>snippet</b> text.</a>
    </body></html>"##;

    #[test]
    fn parses_ddg_results_and_resolves_redirects() {
        let hits = parse_ddg_html(DDG_FIXTURE, 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "The Rust Programming Language");
        assert_eq!(hits[0].url, "https://www.rust-lang.org/");
        assert!(hits[0].snippet.contains("empowering everyone"));
        assert!(!hits[0].snippet.contains('&'));
        // Direct (non-redirect) href passes through untouched.
        assert_eq!(hits[1].url, "https://doc.rust-lang.org/book/");
    }

    #[test]
    fn ddg_limit_is_respected() {
        let hits = parse_ddg_html(DDG_FIXTURE, 1).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn ddg_anomaly_page_is_reported() {
        let err = parse_ddg_html("oops anomaly detected", 10).unwrap_err();
        assert_eq!(err.code, "WEB_RATE_LIMITED");
    }

    #[test]
    fn ddg_page_without_results_is_empty_not_error() {
        let hits = parse_ddg_html("<html><body>No results here</body></html>", 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn ddg_legacy_markup_still_parses() {
        let hits = parse_ddg_html(DDG_LEGACY_FIXTURE, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Old Result");
        assert_eq!(hits[0].url, "https://old.example/");
        assert!(hits[0].snippet.contains("Legacy snippet"));
    }

    #[test]
    fn parses_wikipedia_search() {
        let body = r#"{"pages":[
            {"id":1,"key":"Rust_(programming_language)","title":"Rust (programming language)",
             "excerpt":"<span class=\"searchmatch\">Rust</span> is a systems language","description":""},
            {"id":2,"key":"Rust_(film)","title":"Rust (film)","excerpt":"A movie","description":""}
        ]}"#;
        let hits = parse_wikipedia_json(body, 10, "https://en.wikipedia.org").unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].url,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        );
        assert_eq!(hits[0].snippet, "Rust is a systems language");
    }

    #[test]
    fn parses_exa_results_and_truncates_text() {
        let long = "x".repeat(1000);
        let body = format!(
            r#"{{"results":[{{"title":"Rust","url":"https://www.rust-lang.org/","text":"{long}"}},
            {{"url":"https://example.com/","text":""}}]}}"#
        );
        let hits = parse_exa_json(&body, 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].snippet.chars().count(), 400);
        assert_eq!(hits[1].title, "https://example.com/");
    }

    #[test]
    fn parses_firecrawl_search() {
        let body = r#"{"success":true,"data":{"web":[
            {"title":"Rust","url":"https://www.rust-lang.org/","description":"A language empowering everyone"}
        ]}}"#;
        let hits = parse_firecrawl_search_json(body, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].snippet, "A language empowering everyone");
    }

    #[test]
    fn parses_firecrawl_scrape() {
        let body =
            r##"{"success":true,"data":{"markdown":"# Rust\nHello","metadata":{"title":"Rust"}}}"##;
        let page = parse_firecrawl_scrape_json(body, "https://www.rust-lang.org/").unwrap();
        assert_eq!(page.title.as_deref(), Some("Rust"));
        assert!(page.content.starts_with("# Rust"));
        let empty = r#"{"success":true,"data":{}}"#;
        assert!(parse_firecrawl_scrape_json(empty, "https://x").is_err());
    }

    #[test]
    fn crawl_urls_are_validated() {
        assert!(validate_crawl_url("https://example.com/page").is_ok());
        assert!(validate_crawl_url("http://example.com/page").is_ok());
        assert!(validate_crawl_url("ftp://example.com").is_err());
        assert!(validate_crawl_url("https://user:pass@example.com").is_err());
        assert!(validate_crawl_url("not a url").is_err());
    }

    #[test]
    fn keyed_providers_refuse_to_run_without_keys() {
        let keys = WebSearchKeys::default();
        let err = keys.for_provider("exa").unwrap_err();
        assert_eq!(err.code, "WEB_NO_KEY");
        // Blank strings count as missing, not present.
        let keys = WebSearchKeys {
            exa_api_key: Some("   ".into()),
            firecrawl_api_key: None,
        };
        assert!(keys.for_provider("exa").is_err());
        assert!(keys.for_provider("duckduckgo").is_ok());
    }

    #[test]
    fn unknown_providers_are_rejected() {
        let engine = WebSearchEngine::new();
        let keys = WebSearchKeys::default();
        let err = engine.search("not-a-provider", "q", 5, &keys).unwrap_err();
        assert_eq!(err.code, "WEB_UNKNOWN_PROVIDER");
        let err = engine
            .read_page("https://example.com", "duckduckgo", &keys)
            .unwrap_err();
        assert_eq!(err.code, "WEB_UNKNOWN_PROVIDER");
    }

    // ---- HTTP-level tests against a loopback server ------------------
    // These exercise the real request path (headers, status handling,
    // body cap) with canned responses.

    /// Serve one canned HTTP response on a random loopback port.
    fn spawn_mock(status: &str, content_type: &str, extra_headers: &str, body: &[u8]) -> Bases {
        use std::io::Write as _;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = body.to_vec();
        let status = status.to_string();
        let ct = content_type.to_string();
        let extra = extra_headers.to_string();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n{}",
                status,
                ct,
                body.len(),
                extra
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&body);
        });
        Bases {
            ddg: format!("http://{addr}"),
            wikipedia: format!("http://{addr}"),
            jina: format!("http://{addr}"),
            exa: format!("http://{addr}"),
            firecrawl: format!("http://{addr}"),
        }
    }

    #[test]
    fn ddg_search_end_to_end_over_http() {
        let bases = spawn_mock(
            "200 OK",
            "text/html; charset=UTF-8",
            "",
            DDG_FIXTURE.as_bytes(),
        );
        let engine = WebSearchEngine::with_bases(bases);
        let hits = engine
            .search("duckduckgo", "rust", 10, &WebSearchKeys::default())
            .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].url, "https://www.rust-lang.org/");
    }

    #[test]
    fn exa_search_end_to_end_with_key() {
        let body = br#"{"results":[{"title":"Rust","url":"https://www.rust-lang.org/","text":"Rust is a language"}]}"#;
        let bases = spawn_mock("200 OK", "application/json", "", body);
        let engine = WebSearchEngine::with_bases(bases);
        let keys = WebSearchKeys {
            exa_api_key: Some("test-key".into()),
            firecrawl_api_key: None,
        };
        let hits = engine.search("exa", "rust", 5, &keys).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Rust");
    }

    #[test]
    fn firecrawl_scrape_end_to_end() {
        let body =
            br##"{"success":true,"data":{"markdown":"# Page","metadata":{"title":"Page"}}}"##;
        let bases = spawn_mock("200 OK", "application/json", "", body);
        let engine = WebSearchEngine::with_bases(bases);
        let keys = WebSearchKeys {
            exa_api_key: None,
            firecrawl_api_key: Some("fc-key".into()),
        };
        let page = engine
            .read_page("https://example.com/doc", "firecrawl", &keys)
            .unwrap();
        assert_eq!(page.title.as_deref(), Some("Page"));
        assert_eq!(page.content, "# Page");
    }

    #[test]
    fn jina_read_end_to_end_json() {
        let body = br##"{"code":200,"data":{"title":"Example Page","url":"https://example.com","content":"# Example\n\nHello world"}}"##;
        let bases = spawn_mock("200 OK", "application/json", "", body);
        let engine = WebSearchEngine::with_bases(bases);
        let page = engine
            .read_page("https://example.com", "jina", &WebSearchKeys::default())
            .unwrap();
        assert_eq!(page.title.as_deref(), Some("Example Page"));
        assert!(page.content.contains("Hello world"));
    }

    #[test]
    fn jina_read_falls_back_to_plain_text() {
        let bases = spawn_mock(
            "200 OK",
            "text/plain",
            "X-Title: Example Page\r\n",
            b"Title: Example\nMarkdown Content:\nHello world",
        );
        let engine = WebSearchEngine::with_bases(bases);
        let page = engine
            .read_page("https://example.com", "jina", &WebSearchKeys::default())
            .unwrap();
        assert_eq!(page.title.as_deref(), Some("Example Page"));
        assert!(page.content.contains("Hello world"));
    }

    #[test]
    fn http_429_maps_to_rate_limited() {
        let bases = spawn_mock("429 Too Many Requests", "text/plain", "", b"slow down");
        let engine = WebSearchEngine::with_bases(bases);
        let err = engine
            .search("duckduckgo", "q", 5, &WebSearchKeys::default())
            .unwrap_err();
        assert_eq!(err.code, "WEB_RATE_LIMITED");
    }

    #[test]
    fn http_401_maps_to_auth_failed() {
        let bases = spawn_mock("401 Unauthorized", "text/plain", "", b"bad key");
        let engine = WebSearchEngine::with_bases(bases);
        let keys = WebSearchKeys {
            exa_api_key: Some("wrong".into()),
            firecrawl_api_key: None,
        };
        let err = engine.search("exa", "q", 5, &keys).unwrap_err();
        assert_eq!(err.code, "WEB_AUTH_FAILED");
    }

    #[test]
    fn oversized_response_is_refused() {
        let big = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        let bases = spawn_mock("200 OK", "text/plain", "", &big);
        let engine = WebSearchEngine::with_bases(bases);
        let err = engine
            .read_page("https://example.com", "jina", &WebSearchKeys::default())
            .unwrap_err();
        assert_eq!(err.code, "INVALID_INPUT");
    }
}
