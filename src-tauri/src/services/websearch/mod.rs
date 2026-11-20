//! Web search and page reading over public HTTP APIs. Every provider works
//! without a key: DuckDuckGo results, Wikipedia, the Jina Reader, Exa
//! (through its public MCP endpoint, at a free rate limit) and Firecrawl
//! (its keyless free tier, or a Firecrawl server on this computer). A key
//! pasted into Settings raises Exa's and Firecrawl's limits. Keys live in
//! config.json on this machine only, travel only to their own provider, and
//! are excluded from backups unless explicitly included. Every request is
//! an explicit user action — nothing here runs in the background.
//!
//! The base URLs are constructor parameters so tests can point the engine
//! at loopback servers; parsing lives in pure functions tested without any
//! network.

use crate::app::error::AppError;
use serde::Serialize;
use serde_json::{Value, json};
use std::io::Read as _;
use std::time::Duration;

/// Cap on any single response body, so a broken or hostile endpoint cannot
/// stream unbounded data into memory.
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (compatible; LocalAide/",
    env!("CARGO_PKG_VERSION"),
    "; +https://github.com/omarjson/LocalAide)"
);
/// The order "Automatic" tries providers in. Each is keyless.
const AUTO_SEARCH_ORDER: [&str; 3] = ["duckduckgo", "exa", "wikipedia"];

/// One selectable provider, as the Settings screen renders it.
#[derive(Debug, Clone, Serialize)]
pub struct WebProvider {
    pub id: &'static str,
    pub label: &'static str,
    /// True when the provider cannot work at all without a key. No shipped
    /// provider needs one; the field stays for the Settings screen.
    pub needs_key: bool,
    /// True when a key can be added to raise the free limits.
    pub takes_key: bool,
    /// "search", "crawl", or "search+crawl".
    pub supports: &'static str,
    pub note: &'static str,
}

pub const PROVIDER_TABLE: &[WebProvider] = &[
    WebProvider {
        id: "auto",
        label: "Automatic",
        needs_key: false,
        takes_key: false,
        supports: "search",
        note: "Tries DuckDuckGo, then Exa, then Wikipedia, and uses the first that answers.",
    },
    WebProvider {
        id: "duckduckgo",
        label: "DuckDuckGo",
        needs_key: false,
        takes_key: false,
        supports: "search",
        note: "Private web search. No key.",
    },
    WebProvider {
        id: "exa",
        label: "Exa",
        needs_key: false,
        takes_key: true,
        supports: "search+crawl",
        note: "Search built for AI. Works without a key at a free rate limit; a key raises it.",
    },
    WebProvider {
        id: "firecrawl",
        label: "Firecrawl",
        needs_key: false,
        takes_key: true,
        supports: "search+crawl",
        note: "Search and clean page text. Works without a key (a small free allowance per computer), with a key, or on your own computer.",
    },
    WebProvider {
        id: "wikipedia",
        label: "Wikipedia",
        needs_key: false,
        takes_key: false,
        supports: "search",
        note: "Encyclopedia articles only. No key.",
    },
    WebProvider {
        id: "jina",
        label: "Jina Reader",
        needs_key: false,
        takes_key: false,
        supports: "crawl",
        note: "Reads a page as text. No key.",
    },
];

/// Provider ids accepted by the engine, in one place for validation.
fn known_provider(id: &str) -> bool {
    PROVIDER_TABLE.iter().any(|p| p.id == id)
}

/// What the user configured, read from config per request.
#[derive(Debug, Clone, Default)]
pub struct WebSearchKeys {
    pub exa_api_key: Option<String>,
    pub firecrawl_api_key: Option<String>,
    /// A Firecrawl server the user runs, for example
    /// `http://localhost:3002`. None = Firecrawl's own service.
    pub firecrawl_base_url: Option<String>,
}

impl WebSearchKeys {
    /// The stored key for a provider, when one is set and not blank.
    pub(crate) fn key(&self, id: &str) -> Option<&str> {
        let key = match id {
            "exa" => self.exa_api_key.as_deref(),
            "firecrawl" => self.firecrawl_api_key.as_deref(),
            _ => None,
        };
        key.map(str::trim).filter(|k| !k.is_empty())
    }

    fn firecrawl_base<'a>(&'a self, default: &'a str) -> &'a str {
        self.firecrawl_base_url
            .as_deref()
            .map(|u| u.trim().trim_end_matches('/'))
            .filter(|u| !u.is_empty())
            .unwrap_or(default)
    }

    /// True when page reading should go to Firecrawl by default: the user
    /// added a key or runs their own Firecrawl server.
    pub(crate) fn prefers_firecrawl(&self) -> bool {
        self.key("firecrawl").is_some()
            || self
                .firecrawl_base_url
                .as_deref()
                .is_some_and(|u| !u.trim().is_empty())
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
    /// Exa's public MCP endpoint (the keyless path).
    pub exa_mcp: String,
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
            exa_mcp: "https://mcp.exa.ai".into(),
            firecrawl: "https://api.firecrawl.dev".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WebHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
    /// The provider that found this result.
    pub source: String,
}

impl WebHit {
    fn new(title: String, url: String, snippet: String) -> Self {
        Self {
            title,
            url,
            snippet,
            source: String::new(),
        }
    }
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

    /// Search the web with the chosen provider. Results become links the user
    /// can open, so only plain http(s) addresses are kept.
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
        if provider != "auto" {
            return self.search_one(provider, query, limit, keys);
        }
        // Automatic: the first provider with results wins. A provider that
        // fails or finds nothing hands over to the next one.
        let mut last_error = None;
        for id in AUTO_SEARCH_ORDER {
            match self.search_one(id, query, limit, keys) {
                Ok(hits) if !hits.is_empty() => return Ok(hits),
                Ok(_) => {}
                Err(e) => {
                    log::warn!("Web search via {id} failed: {e}");
                    last_error = Some(e);
                }
            }
        }
        match last_error {
            Some(e) => Err(e),
            None => Ok(Vec::new()),
        }
    }

    fn search_one(
        &self,
        provider: &str,
        query: &str,
        limit: u32,
        keys: &WebSearchKeys,
    ) -> Result<Vec<WebHit>, AppError> {
        let base = match provider {
            "duckduckgo" => self.bases.ddg.clone(),
            "wikipedia" => self.bases.wikipedia.clone(),
            "exa" => self.bases.exa.clone(),
            "firecrawl" => keys.firecrawl_base(&self.bases.firecrawl).to_string(),
            _ => String::new(),
        };
        let audit = crate::services::cloud_audit::start(
            provider,
            &base,
            "",
            "web search",
            query,
            query.len(),
            None,
        );
        let found = self.search_unfiltered(provider, query, limit, keys);
        match &found {
            Ok(h) => audit.finish(true, &format!("{} results", h.len())),
            Err(e) => audit.finish(false, &e.message),
        }
        let mut hits = found?;
        hits.retain(|hit| validate_crawl_url(&hit.url).is_ok());
        for hit in &mut hits {
            hit.source = provider.to_string();
        }
        Ok(hits)
    }

    fn search_unfiltered(
        &self,
        provider: &str,
        query: &str,
        limit: u32,
        keys: &WebSearchKeys,
    ) -> Result<Vec<WebHit>, AppError> {
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
            "exa" => match keys.key("exa") {
                Some(key) => {
                    let body = json!({
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
                None => {
                    let text = self.mcp_call(
                        &self.bases.exa_mcp,
                        "web_search_advanced_exa",
                        json!({
                            "query": query,
                            "numResults": limit,
                            "textMaxCharacters": 500
                        }),
                    )?;
                    Ok(parse_exa_mcp_search(&text, limit))
                }
            },
            "firecrawl" => {
                let base = keys.firecrawl_base(&self.bases.firecrawl);
                let body = json!({"query": query, "limit": limit}).to_string();
                let auth = keys.key("firecrawl").map(|k| format!("Bearer {k}"));
                let headers: Vec<(&str, &str)> = auth
                    .as_deref()
                    .map(|a| vec![("Authorization", a)])
                    .unwrap_or_default();
                let response = self.post_json(&format!("{base}/v2/search"), &headers, &body)?;
                parse_firecrawl_search_json(&response, limit)
            }
            // Crawl-only providers cannot serve searches.
            _ => Err(AppError::new(
                "WEB_UNKNOWN_PROVIDER",
                format!("'{provider}' cannot search the web."),
            )),
        }
    }

    /// Read one page as text. "auto" uses Firecrawl when the user added a
    /// key or runs their own Firecrawl, otherwise the Jina Reader, and falls
    /// back to Exa when Jina fails.
    pub fn read_page(
        &self,
        url: &str,
        provider: &str,
        keys: &WebSearchKeys,
    ) -> Result<WebPage, AppError> {
        // Send the parsed form, so what was checked is exactly what is sent.
        let checked = validate_crawl_url(url)?;
        let url = checked.as_str();
        // "auto" picks a reader and comes back here; the reader call is logged.
        let audit = (provider != "auto").then(|| {
            crate::services::cloud_audit::start(
                provider,
                url,
                "",
                "page read",
                url,
                url.len(),
                None,
            )
        });
        let result = self.read_page_with(url, provider, keys);
        if let Some(a) = audit {
            match &result {
                Ok(_) => a.finish(true, "read"),
                Err(e) => a.finish(false, &e.message),
            }
        }
        result
    }

    fn read_page_with(
        &self,
        url: &str,
        provider: &str,
        keys: &WebSearchKeys,
    ) -> Result<WebPage, AppError> {
        match provider {
            "auto" => {
                if keys.prefers_firecrawl() {
                    return self.read_page(url, "firecrawl", keys);
                }
                match self.read_page(url, "jina", keys) {
                    Ok(page) => Ok(page),
                    Err(jina_error) => {
                        log::warn!("Jina Reader failed, trying Exa: {jina_error}");
                        self.read_page(url, "exa", keys).map_err(|_| jina_error)
                    }
                }
            }
            "firecrawl" => {
                let base = keys.firecrawl_base(&self.bases.firecrawl);
                let body = json!({
                    "url": url,
                    "formats": ["markdown"],
                    "onlyMainContent": true
                })
                .to_string();
                let auth = keys.key("firecrawl").map(|k| format!("Bearer {k}"));
                let headers: Vec<(&str, &str)> = auth
                    .as_deref()
                    .map(|a| vec![("Authorization", a)])
                    .unwrap_or_default();
                let response = self.post_json(&format!("{base}/v2/scrape"), &headers, &body)?;
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
            "exa" => {
                let text = self.mcp_call(
                    &self.bases.exa_mcp,
                    "web_fetch_exa",
                    json!({"urls": [url], "maxCharacters": 20000}),
                )?;
                Ok(parse_exa_mcp_page(&text, url))
            }
            other => Err(AppError::new(
                "WEB_UNKNOWN_PROVIDER",
                format!("'{other}' cannot read pages. Use Firecrawl, Jina Reader or Exa."),
            )),
        }
    }

    /// One MCP tool call over Streamable HTTP: initialize, the initialized
    /// notification, then `tools/call`. Answers may come as JSON or as
    /// server-sent events. Returns the tool's text output.
    fn mcp_call(&self, base: &str, tool: &str, arguments: Value) -> Result<String, AppError> {
        let url = format!(
            "{}/mcp?tools=web_search_advanced_exa,web_fetch_exa",
            base.trim_end_matches('/')
        );
        let init = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "LocalAide", "version": env!("CARGO_PKG_VERSION")}
            }
        });
        let (status, session, _) = self.mcp_post(&url, None, &init)?;
        check_status(status)?;
        let session = session.as_deref();
        // The notification has no answer worth reading; servers reply 202.
        let _ = self.mcp_post(
            &url,
            session,
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        );
        let call = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": tool, "arguments": arguments}
        });
        let (status, _, body) = self.mcp_post(&url, session, &call)?;
        check_status(status)?;
        parse_mcp_result(&body)
    }

    fn mcp_post(
        &self,
        url: &str,
        session: Option<&str>,
        payload: &Value,
    ) -> Result<(reqwest::StatusCode, Option<String>, String), AppError> {
        let mut request = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("User-Agent", USER_AGENT);
        if let Some(id) = session {
            request = request.header("Mcp-Session-Id", id);
        }
        let response = request
            .body(payload.to_string())
            .send()
            .map_err(|e| AppError::internal(format!("Web request failed: {e}")))?;
        let status = response.status();
        let session = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = if status.is_success() {
            read_capped(response)?
        } else {
            String::new()
        };
        Ok((status, session, body))
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
            .header("User-Agent", USER_AGENT)
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
            .header("User-Agent", USER_AGENT);
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
            .header("User-Agent", USER_AGENT)
            .header("Accept", "text/html")
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

/// Crawl targets must be plain http(s) URLs on a public host, without
/// embedded credentials. The address goes to an online reader (Jina or
/// Firecrawl), so addresses inside the user's own computer or network never
/// leave it. Returns the parsed URL, which is what gets sent.
pub fn validate_crawl_url(url: &str) -> Result<url::Url, AppError> {
    let parsed = url::Url::parse(url)
        .map_err(|_| AppError::invalid_input("That is not a valid URL to read."))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::invalid_input(
            "Do not put a username or password in the URL.",
        ));
    }
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err(AppError::invalid_input(
            "Only http and https pages can be read.",
        ));
    }
    if is_private_host(&parsed) {
        return Err(AppError::invalid_input(
            "Pages on this computer or your local network cannot be read here, because the address would be sent to an online reader.",
        ));
    }
    Ok(parsed)
}

/// Hosts that only make sense inside the user's own computer or network.
fn is_private_host(url: &url::Url) -> bool {
    match url.host() {
        None => true,
        Some(url::Host::Domain(name)) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            const LOCAL_SUFFIXES: [&str; 6] = [
                ".localhost",
                ".local",
                ".internal",
                ".lan",
                ".home.arpa",
                ".intranet",
            ];
            name == "localhost"
                || !name.contains('.')
                || LOCAL_SUFFIXES.iter().any(|suffix| name.ends_with(*suffix))
        }
        Some(url::Host::Ipv4(ip)) => is_private_v4(ip),
        Some(url::Host::Ipv6(ip)) => {
            let first = ip.segments()[0];
            ip.is_loopback()
                || ip.is_unspecified()
                || (first & 0xfe00) == 0xfc00 // unique local, fc00::/7
                || (first & 0xffc0) == 0xfe80 // link local, fe80::/10
                || ip.to_ipv4_mapped().is_some_and(is_private_v4)
        }
    }
}

fn is_private_v4(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_documentation()
        || (a == 100 && (b & 0xc0) == 64) // shared address space, 100.64.0.0/10
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

/// The value of one attribute in an HTML open tag, with either quote style
/// or none. Attribute names are matched exactly (DDG writes lowercase).
fn tag_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let mut search = 0;
    while let Some(found) = tag[search..].find(name) {
        let at = search + found;
        search = at + name.len();
        // Must be a whole attribute name: preceded by whitespace, followed by '='.
        let before_ok = at > 0 && bytes[at - 1].is_ascii_whitespace();
        let rest = tag[at + name.len()..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let value = rest[1..].trim_start();
        return match value.chars().next() {
            Some(q @ ('"' | '\'')) => value[1..].find(q).map(|end| &value[1..1 + end]),
            Some(_) => {
                let end = value
                    .find(|c: char| c.is_ascii_whitespace() || c == '>')
                    .unwrap_or(value.len());
                Some(&value[..end])
            }
            None => None,
        };
    }
    None
}

fn tag_has_class(tag: &str, classes: &[&str]) -> bool {
    tag_attr(tag, "class")
        .map(|value| value.split_ascii_whitespace().any(|c| classes.contains(&c)))
        .unwrap_or(false)
}

const DDG_TITLE_CLASSES: [&str; 2] = ["result-link", "result__a"];
const DDG_SNIPPET_CLASSES: [&str; 2] = ["result-snippet", "result__snippet"];

/// Every open tag `<name ...>` in `body`, as (start, end-after-'>').
fn open_tags<'a>(body: &'a str, name: &'a str) -> impl Iterator<Item = (usize, usize)> + 'a {
    let needle = format!("<{name}");
    let mut from = 0;
    std::iter::from_fn(move || {
        loop {
            let found = body[from..].find(&needle)?;
            let start = from + found;
            let after = start + needle.len();
            from = after;
            // "<a" must be followed by whitespace or '>', so "<abbr" is skipped.
            match body.as_bytes().get(after) {
                Some(b) if b.is_ascii_whitespace() || *b == b'>' => {}
                _ => continue,
            }
            let end = body[after..].find('>')? + after + 1;
            return Some((start, end));
        }
    })
}

/// Parse a DuckDuckGo results page (the lite table or the full HTML page).
/// Pure function so tests need no network. Result titles are anchors with
/// a `result-link` (lite) or `result__a` (full page) class; snippets are
/// elements with `result-snippet` or `result__snippet`. Attributes can come
/// in any order and with either quote style: the lite page writes
/// `href="…" class='result-link'`.
pub fn parse_ddg_html(body: &str, limit: usize) -> Result<Vec<WebHit>, AppError> {
    let lowered = body.to_ascii_lowercase();
    if lowered.contains("anomaly-modal")
        || lowered.contains("unusual traffic")
        || lowered.contains("bots use duckduckgo too")
    {
        return Err(AppError::new(
            "WEB_RATE_LIMITED",
            "DuckDuckGo asked for a check that LocalAide cannot pass. Try again later, or choose Automatic or Exa in Settings → Web search.",
        ));
    }

    let anchors: Vec<(usize, usize)> = open_tags(body, "a")
        .filter(|(start, end)| tag_has_class(&body[*start..*end], &DDG_TITLE_CLASSES))
        .collect();

    let mut hits = Vec::new();
    for (i, (start, open_end)) in anchors.iter().enumerate() {
        if hits.len() >= limit {
            break;
        }
        let tag = &body[*start..*open_end];
        let Some(href) = tag_attr(tag, "href") else {
            continue;
        };
        let Some(close) = body[*open_end..].find("</a>") else {
            continue;
        };
        let title = decode_entities(&strip_tags(&body[*open_end..open_end + close]))
            .trim()
            .to_string();

        // The snippet is the next snippet element before the next result.
        let window_end = anchors.get(i + 1).map(|(s, _)| *s).unwrap_or(body.len());
        let window_start = open_end + close + 4;
        let snippet = if window_start < window_end {
            let window = &body[window_start..window_end];
            ["td", "div", "a", "span"]
                .iter()
                .flat_map(|name| open_tags(window, name))
                .filter(|(s, e)| tag_has_class(&window[*s..*e], &DDG_SNIPPET_CLASSES))
                .min_by_key(|(s, _)| *s)
                .map(|(_, e)| {
                    let tail = &window[e..];
                    let end = ["</td>", "</div>", "</a>", "</span>"]
                        .iter()
                        .filter_map(|c| tail.find(c))
                        .min()
                        .unwrap_or(tail.len());
                    collapse_space(&decode_entities(&strip_tags(&tail[..end])))
                })
                .unwrap_or_default()
        } else {
            String::new()
        };

        if !title.is_empty() {
            hits.push(WebHit::new(title, resolve_ddg_href(href), snippet));
        }
    }
    Ok(hits)
}

fn collapse_space(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The text of an MCP `tools/call` answer, sent as plain JSON or as
/// server-sent events (`data:` lines).
pub fn parse_mcp_result(body: &str) -> Result<String, AppError> {
    let trimmed = body.trim();
    let message: Value = if trimmed.starts_with('{') {
        serde_json::from_str(trimmed).map_err(|_| {
            AppError::internal("The search service sent an answer LocalAide could not read.")
        })?
    } else {
        trimmed
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .filter_map(|data| serde_json::from_str::<Value>(data.trim()).ok())
            .rfind(|v| v.get("result").is_some() || v.get("error").is_some())
            .ok_or_else(|| AppError::internal("The search service sent an empty answer."))?
    };
    if let Some(error) = message.get("error") {
        let text = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        if text.to_ascii_lowercase().contains("rate") {
            return Err(AppError::new(
                "WEB_RATE_LIMITED",
                "The free search limit is used up for now. Wait a little, or add a key in Settings → Web search.",
            ));
        }
        return Err(AppError::internal(format!(
            "The search service said: {text}"
        )));
    }
    let result = &message["result"];
    let text = result["content"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        let lower = text.to_ascii_lowercase();
        if lower.contains("rate limit")
            || lower.contains("429")
            || (lower.contains("free") && lower.contains("limit"))
        {
            return Err(AppError::new(
                "WEB_RATE_LIMITED",
                "The free search limit is used up for now. Wait a little, or add a key in Settings → Web search.",
            ));
        }
        let short: String = text.chars().take(300).collect();
        return Err(AppError::internal(format!(
            "The search service said: {short}"
        )));
    }
    Ok(text)
}

/// Exa MCP search output: a JSON document with `results`, or the plain-text
/// form with `Title:` and `URL:` lines.
pub fn parse_exa_mcp_search(text: &str, limit: usize) -> Vec<WebHit> {
    if let Ok(json) = serde_json::from_str::<Value>(text.trim())
        && let Some(results) = json.get("results").and_then(Value::as_array)
    {
        return results
            .iter()
            .filter_map(|r| {
                let url = r.get("url").and_then(Value::as_str)?.to_string();
                let title = r
                    .get("title")
                    .and_then(Value::as_str)
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or(&url)
                    .trim()
                    .to_string();
                let snippet = ["summary", "text"]
                    .iter()
                    .find_map(|k| r.get(*k).and_then(Value::as_str))
                    .map(str::to_string)
                    .or_else(|| {
                        r.get("highlights").and_then(Value::as_array).map(|h| {
                            h.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join(" … ")
                        })
                    })
                    .unwrap_or_default();
                Some(WebHit::new(
                    title,
                    url,
                    truncate_chars(&collapse_space(&snippet), 400),
                ))
            })
            .take(limit)
            .collect();
    }

    let mut hits = Vec::new();
    let mut title: Option<String> = None;
    let mut url: Option<String> = None;
    let mut snippet = String::new();
    let mut flush = |title: &mut Option<String>, url: &mut Option<String>, snippet: &mut String| {
        if let Some(u) = url.take() {
            let t = title.take().unwrap_or_else(|| u.clone());
            hits.push(WebHit::new(
                t,
                u,
                truncate_chars(&collapse_space(snippet), 400),
            ));
        }
        title.take();
        snippet.clear();
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(t) = line.strip_prefix("Title:") {
            flush(&mut title, &mut url, &mut snippet);
            title = Some(t.trim().to_string());
        } else if let Some(u) = line.strip_prefix("URL:") {
            url = Some(u.trim().to_string());
        } else if line.starts_with("Published")
            || line.starts_with("Author")
            || line.starts_with("ID:")
        {
            continue;
        } else if !line.is_empty() && url.is_some() && snippet.len() < 600 {
            let text = line
                .strip_prefix("Text:")
                .or_else(|| line.strip_prefix("Highlights:"))
                .or_else(|| line.strip_prefix("Summary:"))
                .unwrap_or(line);
            snippet.push_str(text.trim());
            snippet.push(' ');
        }
    }
    flush(&mut title, &mut url, &mut snippet);
    hits.truncate(limit);
    hits
}

/// Exa MCP page output: text, optionally led by `Title:` and `URL:` lines.
pub fn parse_exa_mcp_page(text: &str, url: &str) -> WebPage {
    let mut title = None;
    let mut body = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if title.is_none()
            && let Some(t) = trimmed.strip_prefix("Title:")
        {
            title = Some(t.trim().to_string());
            continue;
        }
        if body.is_empty() && (trimmed.starts_with("URL:") || trimmed.starts_with("ID:")) {
            continue;
        }
        body.push(line);
    }
    let content = body.join("\n").trim().to_string();
    WebPage {
        title: title.filter(|t| !t.is_empty()),
        url: url.to_string(),
        content: content
            .strip_prefix("Text:")
            .map(|c| c.trim().to_string())
            .unwrap_or(content),
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{}…", cut.trim_end())
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
                source: String::new(),
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
                source: String::new(),
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
            source: String::new(),
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

    /// The real lite page: `href` before `class`, single-quoted classes,
    /// snippets in the next table row.
    const DDG_LITE_FIXTURE: &str = r##"<table border="0">
      <tr>
        <td valign="top">1.&nbsp;</td>
        <td>
          <a rel="nofollow" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F&amp;rut=1" class='result-link'>Rust Programming Language</a>
        </td>
      </tr>
      <tr>
        <td>&nbsp;&nbsp;&nbsp;</td>
        <td class='result-snippet'>
          A language empowering <b>everyone</b>
          to build reliable software.
        </td>
      </tr>
      <tr><td>&nbsp;</td><td><span class='link-text'>www.rust-lang.org</span></td></tr>
      <tr>
        <td valign="top">2.&nbsp;</td>
        <td><a rel="nofollow" href="https://en.wikipedia.org/wiki/Rust" class='result-link'>Rust - Wikipedia</a></td>
      </tr>
      <tr><td>&nbsp;</td><td class='result-snippet'>Rust is a language.</td></tr>
    </table>"##;

    #[test]
    fn parses_the_real_lite_layout() {
        let hits = parse_ddg_html(DDG_LITE_FIXTURE, 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Rust Programming Language");
        assert_eq!(hits[0].url, "https://www.rust-lang.org/");
        assert_eq!(
            hits[0].snippet,
            "A language empowering everyone to build reliable software."
        );
        assert_eq!(hits[1].url, "https://en.wikipedia.org/wiki/Rust");
        assert_eq!(hits[1].snippet, "Rust is a language.");
    }

    #[test]
    fn attribute_reader_handles_quotes_and_order() {
        let tag = r#"<a rel="nofollow" href='https://x.example/' class="a result-link b">"#;
        assert_eq!(tag_attr(tag, "href"), Some("https://x.example/"));
        assert!(tag_has_class(tag, &DDG_TITLE_CLASSES));
        assert!(!tag_has_class(
            r#"<a data-class="result-link">"#,
            &DDG_TITLE_CLASSES
        ));
        assert_eq!(
            tag_attr("<td class=result-snippet>", "class"),
            Some("result-snippet")
        );
    }

    #[test]
    fn mcp_answers_parse_as_json_or_events() {
        let json =
            r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"hello"}]}}"#;
        assert_eq!(parse_mcp_result(json).unwrap(), "hello");
        let sse = format!("event: message\ndata: {json}\n\n");
        assert_eq!(parse_mcp_result(&sse).unwrap(), "hello");
        let err = r#"{"jsonrpc":"2.0","id":2,"result":{"isError":true,"content":[{"type":"text","text":"429 rate limit"}]}}"#;
        assert_eq!(parse_mcp_result(err).unwrap_err().code, "WEB_RATE_LIMITED");
        assert!(parse_mcp_result("").is_err());
    }

    #[test]
    fn exa_mcp_results_parse_in_both_forms() {
        let json = r#"{"results":[{"title":"Rust","url":"https://www.rust-lang.org/","text":"A language"},{"url":"https://b.example/","highlights":["one","two"]}]}"#;
        let hits = parse_exa_mcp_search(json, 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].snippet, "A language");
        assert_eq!(hits[1].title, "https://b.example/");
        assert_eq!(hits[1].snippet, "one … two");
        let text = "Title: Rust\nURL: https://www.rust-lang.org/\nPublished: 2026\nText: A language\nfor everyone\n\nTitle: Book\nURL: https://doc.rust-lang.org/book/\n";
        let hits = parse_exa_mcp_search(text, 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].snippet, "A language for everyone");
        assert_eq!(hits[1].title, "Book");
        let page = parse_exa_mcp_page(
            "Title: Rust\nURL: https://x/\nText: Hello\nworld",
            "https://x/",
        );
        assert_eq!(page.title.as_deref(), Some("Rust"));
        assert_eq!(page.content, "Hello\nworld");
    }

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
        // Entities decode to their characters; no raw markup survives.
        assert!(!hits[0].snippet.contains("&amp;"));
        assert!(hits[0].snippet.contains("software & more"));
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
        let err = parse_ddg_html(
            "<div class=\"anomaly-modal\">bots use DuckDuckGo too</div>",
            10,
        )
        .unwrap_err();
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
    fn addresses_inside_the_users_network_are_not_sent_out() {
        for private in [
            "http://localhost:8080/admin",
            "http://127.0.0.1/",
            "http://10.0.0.5/files",
            "http://192.168.1.1/",
            "http://172.16.4.2/",
            "http://169.254.169.254/latest/meta-data",
            "http://100.64.0.1/",
            "http://[::1]/",
            "http://[fd00::1]/",
            "http://[fe80::1]/",
            "http://[::ffff:192.168.0.1]/",
            "http://nas/share",
            "http://printer.local/",
            "http://wiki.corp.internal/",
            "http://router.lan/",
            "http://app.localhost/",
        ] {
            assert!(
                validate_crawl_url(private).is_err(),
                "{private} must stay home"
            );
        }
        for public in [
            "https://example.com/a",
            "http://93.184.216.34/",
            "https://[2606:4700::1]/",
        ] {
            assert!(validate_crawl_url(public).is_ok(), "{public} is public");
        }
    }

    #[test]
    fn blank_keys_count_as_missing() {
        let keys = WebSearchKeys::default();
        assert_eq!(keys.key("exa"), None);
        let keys = WebSearchKeys {
            exa_api_key: Some("   ".into()),
            firecrawl_api_key: Some(" fc ".into()),
            firecrawl_base_url: None,
        };
        assert_eq!(keys.key("exa"), None);
        assert_eq!(keys.key("firecrawl"), Some("fc"));
        assert_eq!(keys.key("duckduckgo"), None);
        assert!(keys.prefers_firecrawl());
        let local = WebSearchKeys {
            firecrawl_base_url: Some("http://localhost:3002/".into()),
            ..WebSearchKeys::default()
        };
        assert!(local.prefers_firecrawl());
        assert_eq!(
            local.firecrawl_base("https://api.firecrawl.dev"),
            "http://localhost:3002"
        );
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

    type Canned = (&'static str, &'static str, &'static str, Vec<u8>);

    /// Serve one canned HTTP response on a random loopback port.
    fn spawn_mock(
        status: &'static str,
        content_type: &'static str,
        extra_headers: &'static str,
        body: &[u8],
    ) -> Bases {
        spawn_mock_seq(vec![(status, content_type, extra_headers, body.to_vec())]).0
    }

    /// Serve canned responses in order, one connection each, and record the
    /// raw requests so tests can check the headers that were sent.
    fn spawn_mock_seq(
        responses: Vec<Canned>,
    ) -> (Bases, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::Write as _;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (status, ct, extra, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                log.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).to_string());
                // The blank line after the extra headers terminates the header
                // section; without it every client rejects the response.
                let response = format!(
                    "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n",
                    status,
                    ct,
                    body.len(),
                    extra
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        let base = format!("http://{addr}");
        (
            Bases {
                ddg: base.clone(),
                wikipedia: base.clone(),
                jina: base.clone(),
                exa: base.clone(),
                exa_mcp: base.clone(),
                firecrawl: base,
            },
            seen,
        )
    }

    fn mcp_exchange(call_body: &str) -> Vec<Canned> {
        vec![
            (
                "200 OK",
                "application/json",
                "Mcp-Session-Id: s-1\r\n",
                br#"{"jsonrpc":"2.0","id":1,"result":{}}"#.to_vec(),
            ),
            ("202 Accepted", "application/json", "", Vec::new()),
            (
                "200 OK",
                "text/event-stream",
                "",
                format!("event: message\ndata: {call_body}\n\n").into_bytes(),
            ),
        ]
    }

    #[test]
    fn exa_searches_without_a_key_over_mcp() {
        let call = r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"Title: Rust\nURL: https://www.rust-lang.org/\nText: A language"}]}}"#;
        let (bases, seen) = spawn_mock_seq(mcp_exchange(call));
        let engine = WebSearchEngine::with_bases(bases);
        let hits = engine
            .search("exa", "rust", 5, &WebSearchKeys::default())
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "exa");
        let requests = seen.lock().unwrap();
        assert!(requests[0].contains("\"initialize\""));
        assert!(
            requests[2]
                .to_ascii_lowercase()
                .contains("mcp-session-id: s-1")
        );
        assert!(requests[2].contains("web_search_advanced_exa"));
    }

    #[test]
    fn automatic_search_falls_back_when_duckduckgo_finds_nothing() {
        let call = r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"results\":[{\"title\":\"Rust\",\"url\":\"https://www.rust-lang.org/\"}]}"}]}}"#;
        let mut responses: Vec<Canned> = vec![(
            "200 OK",
            "text/html",
            "",
            b"<html>no results</html>".to_vec(),
        )];
        responses.extend(mcp_exchange(call));
        let (bases, _) = spawn_mock_seq(responses);
        let engine = WebSearchEngine::with_bases(bases);
        let hits = engine
            .search("auto", "rust", 5, &WebSearchKeys::default())
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "exa");
    }

    #[test]
    fn firecrawl_sends_no_key_when_none_is_set() {
        let body = br#"{"success":true,"data":{"web":[{"url":"https://www.rust-lang.org/","title":"Rust","description":"A language"}]}}"#;
        let (bases, seen) = spawn_mock_seq(vec![("200 OK", "application/json", "", body.to_vec())]);
        let engine = WebSearchEngine::with_bases(bases);
        let hits = engine
            .search("firecrawl", "rust", 5, &WebSearchKeys::default())
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert!(
            !seen.lock().unwrap()[0]
                .to_ascii_lowercase()
                .contains("authorization")
        );
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
            ..WebSearchKeys::default()
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
            firecrawl_api_key: Some("fc-key".into()),
            ..WebSearchKeys::default()
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
            ..WebSearchKeys::default()
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
