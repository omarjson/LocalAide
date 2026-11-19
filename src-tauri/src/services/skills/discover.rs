//! Find skills and prompts in public GitHub repositories, and import them as
//! local skills.
//!
//! Nothing runs on its own: browsing starts when the user opens a source on
//! the Skills screen. A listing costs one GitHub API call (60 per hour are
//! allowed without an account), and skill files come from
//! raw.githubusercontent.com. Imported skills record their source address and
//! license line.
//!
//! Two layouts are understood:
//! - Skill folders: `skills/<name>/SKILL.md` with `name` and `description`
//!   frontmatter (Anthropic Skills, GitHub Awesome Copilot, and any repo
//!   that follows the same layout).
//! - A CSV of role prompts with `act` and `prompt` columns (prompts.chat).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::split_frontmatter;
use crate::app::error::AppError;

const API_BASE: &str = "https://api.github.com";
const RAW_BASE: &str = "https://raw.githubusercontent.com";
const MAX_LISTING_BYTES: usize = 4 * 1024 * 1024;
/// prompts.chat's CSV is about 11 MB.
const MAX_CSV_BYTES: usize = 24 * 1024 * 1024;
const MAX_SKILL_FILE_BYTES: usize = 256 * 1024;
const MAX_ITEMS: usize = 5000;
const PREVIEW_CHARS: usize = 220;

/// A place skills can come from, shown as a card on the Skills screen.
#[derive(Debug, Clone, Serialize)]
pub struct SkillSource {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// The repository page, for "View on GitHub".
    pub url: &'static str,
    pub license_note: &'static str,
}

pub const SOURCES: &[SkillSource] = &[
    SkillSource {
        id: "anthropic",
        label: "Anthropic Skills",
        description: "Official skill guides for writing, documents, design and more.",
        url: "https://github.com/anthropics/skills",
        license_note: "Each skill names its license. Some are Apache-2.0; the document skills are source-available only.",
    },
    SkillSource {
        id: "copilot",
        label: "GitHub Awesome Copilot",
        description: "Hundreds of community skills for coding, writing and research.",
        url: "https://github.com/github/awesome-copilot",
        license_note: "MIT licensed collection.",
    },
    SkillSource {
        id: "prompts-chat",
        label: "prompts.chat",
        description: "The classic \"Act as…\" prompt collection, with thousands of roles.",
        url: "https://github.com/f/prompts.chat",
        license_note: "CC0: free to use for anything.",
    },
];

/// One skill in a listing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteSkill {
    /// Stable within its source: a folder name or a prompt title.
    pub id: String,
    pub name: String,
    /// What the skill is for, when already known (from its SKILL.md).
    pub description: Option<String>,
    /// A short preview, when the listing has one.
    pub preview: Option<String>,
    /// Page to open in the browser.
    pub url: String,
}

/// A listed skill's description, from `describe`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillDescription {
    pub id: String,
    pub name: String,
    pub description: String,
    pub license: Option<String>,
}

/// At most this many skills are described per call (one page of the list).
const MAX_DESCRIBE: usize = 60;

/// A skill's full text, for preview and import.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteSkillDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    pub body: String,
    pub license: Option<String>,
    pub url: String,
}

/// Where a request points, after validation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    SkillDirs {
        owner: String,
        repo: String,
        dir: String,
    },
    PromptsCsv,
}

impl Target {
    fn resolve(source: &str, repo: Option<&str>) -> Result<(String, Self), AppError> {
        match source {
            "anthropic" => Ok((
                "anthropic".into(),
                Self::SkillDirs {
                    owner: "anthropics".into(),
                    repo: "skills".into(),
                    dir: "skills".into(),
                },
            )),
            "copilot" => Ok((
                "copilot".into(),
                Self::SkillDirs {
                    owner: "github".into(),
                    repo: "awesome-copilot".into(),
                    dir: "skills".into(),
                },
            )),
            "prompts-chat" => Ok(("prompts-chat".into(), Self::PromptsCsv)),
            "github" => {
                let (owner, repo, dir) = parse_repo_ref(repo.unwrap_or(""))?;
                Ok((
                    format!("github:{owner}/{repo}/{dir}"),
                    Self::SkillDirs { owner, repo, dir },
                ))
            }
            other => Err(AppError::invalid_input(format!(
                "Unknown skill source: {other}"
            ))),
        }
    }
}

/// `owner/repo` or `owner/repo/path/to/skills`, optionally as a GitHub
/// address. Only plain names are accepted, so nothing can be smuggled into
/// the request path.
fn parse_repo_ref(input: &str) -> Result<(String, String, String), AppError> {
    let trimmed = input
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("github.com/")
        .trim_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
    let invalid = || {
        AppError::invalid_input(
            "Enter a GitHub repository as owner/name, for example anthropics/skills.",
        )
    };
    if parts.len() < 2 || parts.len() > 8 {
        return Err(invalid());
    }
    let ok = |p: &str| {
        p != "."
            && p != ".."
            && p.len() <= 100
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if !parts.iter().all(|p| ok(p)) {
        return Err(invalid());
    }
    // "tree/<branch>/" in a pasted address is not part of the path.
    let mut rest: Vec<&str> = parts[2..].to_vec();
    if rest.first() == Some(&"tree") && rest.len() >= 2 {
        rest.drain(..2);
    }
    let dir = if rest.is_empty() {
        "skills".to_string()
    } else {
        rest.join("/")
    };
    Ok((parts[0].to_string(), parts[1].to_string(), dir))
}

pub struct SkillDiscovery {
    client: reqwest::Client,
    api_base: String,
    raw_base: String,
    /// Listings and the prompts CSV, kept for this session so browsing and
    /// importing do not repeat downloads.
    cache: Mutex<HashMap<String, Vec<RemoteSkillDetail>>>,
}

impl SkillDiscovery {
    pub fn new() -> Self {
        Self::with_bases(API_BASE.to_string(), RAW_BASE.to_string())
    }

    pub fn with_bases(api_base: String, raw_base: String) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_default();
        Self {
            client,
            api_base,
            raw_base,
            cache: Mutex::new(HashMap::new()),
        }
    }

    async fn get(&self, url: &str, cap: usize, accept: &str) -> Result<String, AppError> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", "LocalAide-skills")
            .header("Accept", accept)
            .send()
            .await
            .map_err(|e| {
                AppError::new(
                    "SKILLS_OFFLINE",
                    "Could not reach GitHub. Check your internet connection.",
                )
                .with_detail(e.to_string())
                .retryable()
            })?;
        let status = response.status();
        if status.as_u16() == 403 || status.as_u16() == 429 {
            return Err(AppError::new(
                "SKILLS_RATE_LIMITED",
                "GitHub limits browsing without an account to 60 lists an hour. Try again later.",
            )
            .retryable());
        }
        if status.as_u16() == 404 {
            return Err(AppError::new(
                "SKILLS_NOT_FOUND",
                "That repository or folder was not found. Check the name; it must be public.",
            ));
        }
        if !status.is_success() {
            return Err(AppError::new(
                "SKILLS_ERROR",
                format!("GitHub answered with an error ({status})."),
            )
            .retryable());
        }
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::internal(e.to_string()))?;
            if body.len() + chunk.len() > cap {
                return Err(AppError::invalid_input("The file is too large to read."));
            }
            body.extend_from_slice(&chunk);
        }
        String::from_utf8(body).map_err(|_| AppError::invalid_input("The file is not text."))
    }

    fn cached(&self, key: &str) -> Option<Vec<RemoteSkillDetail>> {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .cloned()
    }

    fn store(&self, key: &str, items: Vec<RemoteSkillDetail>) {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.to_string(), items);
    }

    /// Everything a source offers. Skill folders list names only; their text
    /// is fetched on preview.
    pub async fn list(
        &self,
        source: &str,
        repo: Option<&str>,
    ) -> Result<Vec<RemoteSkill>, AppError> {
        let (key, target) = Target::resolve(source, repo)?;
        let items = match self.cached(&key) {
            Some(items) => items,
            None => {
                let items = match &target {
                    Target::SkillDirs { owner, repo, dir } => {
                        let url = format!("{}/repos/{owner}/{repo}/contents/{dir}", self.api_base);
                        let body = self
                            .get(&url, MAX_LISTING_BYTES, "application/vnd.github+json")
                            .await?;
                        parse_listing(&body, owner, repo, dir)?
                    }
                    Target::PromptsCsv => {
                        let url = format!("{}/f/prompts.chat/HEAD/prompts.csv", self.raw_base);
                        let body = self.get(&url, MAX_CSV_BYTES, "text/csv").await?;
                        parse_prompts_csv(&body)
                    }
                };
                self.store(&key, items.clone());
                items
            }
        };
        Ok(items
            .iter()
            .map(|d| RemoteSkill {
                id: d.id.clone(),
                name: d.name.clone(),
                description: (!d.description.is_empty())
                    .then(|| truncate(&d.description, PREVIEW_CHARS)),
                preview: if d.body.is_empty() {
                    None
                } else {
                    Some(truncate(&d.body, PREVIEW_CHARS))
                },
                url: d.url.clone(),
            })
            .collect())
    }

    /// Descriptions for listed skills, read from each skill's SKILL.md
    /// frontmatter (eight files at a time, from raw.githubusercontent.com,
    /// which has no hourly API limit). Read files stay cached, so a later
    /// preview or import needs no download. Skills that fail are left out.
    pub async fn describe(
        &self,
        source: &str,
        repo: Option<&str>,
        ids: &[String],
    ) -> Result<Vec<SkillDescription>, AppError> {
        use futures::StreamExt;
        let wanted: Vec<String> = ids.iter().take(MAX_DESCRIBE).cloned().collect();
        let details: Vec<RemoteSkillDetail> = futures::stream::iter(wanted)
            .map(|id| async move { self.detail(source, repo, &id).await.ok() })
            .buffer_unordered(8)
            .filter_map(|d| async move { d })
            .collect()
            .await;
        Ok(details
            .into_iter()
            .map(|d| SkillDescription {
                description: if d.description.is_empty() {
                    truncate(&d.body, PREVIEW_CHARS)
                } else {
                    truncate(&d.description, PREVIEW_CHARS)
                },
                id: d.id,
                name: d.name,
                license: d.license,
            })
            .collect())
    }

    /// The full text of one skill.
    pub async fn detail(
        &self,
        source: &str,
        repo: Option<&str>,
        id: &str,
    ) -> Result<RemoteSkillDetail, AppError> {
        let (key, target) = Target::resolve(source, repo)?;
        if self.cached(&key).is_none() {
            self.list(source, repo).await?;
        }
        let item = self
            .cached(&key)
            .and_then(|items| items.into_iter().find(|i| i.id == id))
            .ok_or_else(|| {
                AppError::invalid_input(
                    "That skill is no longer in the list. Refresh and try again.",
                )
            })?;
        if !item.body.is_empty() {
            return Ok(item);
        }
        let Target::SkillDirs { owner, repo, dir } = target else {
            return Ok(item);
        };
        let url = format!(
            "{}/{owner}/{repo}/HEAD/{dir}/{}/SKILL.md",
            self.raw_base, item.id
        );
        let raw = self.get(&url, MAX_SKILL_FILE_BYTES, "text/plain").await?;
        let detail = parse_skill_md(&raw, &item);
        // Remember the text, so importing after a preview needs no download.
        if let Some(items) = self
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&key)
            && let Some(slot) = items.iter_mut().find(|i| i.id == id)
        {
            *slot = detail.clone();
        }
        Ok(detail)
    }
}

impl Default for SkillDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate(text: &str, max: usize) -> String {
    let flat = super::one_line(text);
    if flat.chars().count() <= max {
        return flat;
    }
    let cut: String = flat.chars().take(max).collect();
    format!("{}…", cut.trim_end())
}

/// "pdf-tools" → "Pdf tools".
fn pretty_name(id: &str) -> String {
    let words = id.replace(['-', '_'], " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// The GitHub contents API listing of a skills folder: one entry per
/// sub-folder.
fn parse_listing(
    body: &str,
    owner: &str,
    repo: &str,
    dir: &str,
) -> Result<Vec<RemoteSkillDetail>, AppError> {
    let json: Value = serde_json::from_str(body)
        .map_err(|_| AppError::invalid_input("GitHub sent a listing LocalAide could not read."))?;
    let Value::Array(entries) = json else {
        return Err(AppError::new(
            "SKILLS_NOT_FOUND",
            "That address is a file, not a folder of skills.",
        ));
    };
    let mut out: Vec<RemoteSkillDetail> = entries
        .iter()
        .filter(|e| e.get("type").and_then(Value::as_str) == Some("dir"))
        .filter_map(|e| e.get("name").and_then(Value::as_str))
        .filter(|name| !name.starts_with('.'))
        .take(MAX_ITEMS)
        .map(|name| RemoteSkillDetail {
            id: name.to_string(),
            name: pretty_name(name),
            description: String::new(),
            body: String::new(),
            license: None,
            url: format!("https://github.com/{owner}/{repo}/tree/HEAD/{dir}/{name}"),
        })
        .collect();
    if out.is_empty() {
        return Err(AppError::new(
            "SKILLS_NOT_FOUND",
            "No skill folders were found there. Skills live in folders that each hold a SKILL.md file.",
        ));
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

/// A SKILL.md file: frontmatter `name`, `description` and `license`, then
/// the instructions.
fn parse_skill_md(raw: &str, item: &RemoteSkillDetail) -> RemoteSkillDetail {
    let (front, body) = split_frontmatter(raw);
    let clean = |v: &String| {
        v.trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim()
            .to_string()
    };
    let name = front
        .get("name")
        .map(clean)
        .filter(|n| !n.is_empty())
        .map(|n| pretty_name(&n))
        .unwrap_or_else(|| item.name.clone());
    RemoteSkillDetail {
        id: item.id.clone(),
        name,
        description: front.get("description").map(clean).unwrap_or_default(),
        body: body.trim().to_string(),
        license: front.get("license").map(clean).filter(|l| !l.is_empty()),
        url: item.url.clone(),
    }
}

/// prompts.chat's CSV: `act,prompt,…` with quoted fields that may span lines.
fn parse_prompts_csv(body: &str) -> Vec<RemoteSkillDetail> {
    let rows = parse_csv(body);
    let Some(header) = rows.first() else {
        return Vec::new();
    };
    let col = |name: &str| {
        header
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(name))
    };
    let (Some(act), Some(prompt)) = (col("act"), col("prompt")) else {
        return Vec::new();
    };
    let kind = col("type");
    let mut seen = std::collections::HashSet::new();
    rows.iter()
        .skip(1)
        .filter(|row| {
            kind.and_then(|k| row.get(k))
                .map(|t| t.trim().is_empty() || t.trim().eq_ignore_ascii_case("text"))
                .unwrap_or(true)
        })
        .filter_map(|row| {
            let title = super::one_line(row.get(act)?);
            let text = row.get(prompt)?.trim().to_string();
            if title.is_empty() || text.is_empty() || !seen.insert(title.to_lowercase()) {
                return None;
            }
            Some(RemoteSkillDetail {
                id: title.clone(),
                name: title,
                description: String::new(),
                body: text,
                license: Some("CC0-1.0".to_string()),
                url: "https://prompts.chat".to_string(),
            })
        })
        .take(MAX_ITEMS)
        .collect()
}

/// A small RFC 4180 reader: commas, quotes, doubled quotes, and newlines
/// inside quotes.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_references_are_validated() {
        assert_eq!(
            parse_repo_ref("anthropics/skills").ok(),
            Some(("anthropics".into(), "skills".into(), "skills".into()))
        );
        assert_eq!(
            parse_repo_ref("https://github.com/acme/kit/tree/main/prompts/skills").ok(),
            Some(("acme".into(), "kit".into(), "prompts/skills".into()))
        );
        assert_eq!(
            parse_repo_ref("acme/kit.git").ok(),
            Some(("acme".into(), "kit".into(), "skills".into()))
        );
        assert!(parse_repo_ref("acme").is_err());
        assert!(parse_repo_ref("acme/../etc").is_err());
        assert!(parse_repo_ref("acme/kit?x=1").is_err());
        assert!(parse_repo_ref("acme/kit/a b").is_err());
    }

    #[test]
    fn listings_keep_folders_only() {
        let body = r#"[
            {"name":"pdf","type":"dir"},
            {"name":"README.md","type":"file"},
            {"name":".hidden","type":"dir"},
            {"name":"brand-guidelines","type":"dir"}
        ]"#;
        let items = parse_listing(body, "anthropics", "skills", "skills").unwrap();
        assert_eq!(
            items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            ["brand-guidelines", "pdf"]
        );
        assert_eq!(items[0].name, "Brand guidelines");
        assert_eq!(
            items[1].url,
            "https://github.com/anthropics/skills/tree/HEAD/skills/pdf"
        );
        assert!(parse_listing(r#"{"type":"file"}"#, "a", "b", "c").is_err());
        assert!(parse_listing("[]", "a", "b", "c").is_err());
    }

    #[test]
    fn skill_md_frontmatter_is_read() {
        let item = RemoteSkillDetail {
            id: "pdf".into(),
            name: "Pdf".into(),
            description: String::new(),
            body: String::new(),
            license: None,
            url: "u".into(),
        };
        let raw = "---\nname: pdf\ndescription: \"Work with PDF files.\"\nlicense: Apache-2.0\n---\n\n# PDF\n\nSteps.";
        let detail = parse_skill_md(raw, &item);
        assert_eq!(detail.name, "Pdf");
        assert_eq!(detail.description, "Work with PDF files.");
        assert_eq!(detail.license.as_deref(), Some("Apache-2.0"));
        assert_eq!(detail.body, "# PDF\n\nSteps.");
    }

    #[test]
    fn prompts_csv_handles_quotes_and_newlines() {
        let csv = "act,prompt,for_devs,type,contributor\n\
Linux Terminal,\"I want you to act as a \"\"terminal\"\".\nReply in code.\",TRUE,TEXT,a\n\
Painter,\"Draw it\",FALSE,IMAGE,b\n\
linux terminal,\"duplicate\",TRUE,TEXT,c\n\
Chef,Cook simply,FALSE,TEXT,d";
        let items = parse_prompts_csv(csv);
        assert_eq!(items.len(), 2, "image prompts and duplicates are skipped");
        assert_eq!(items[0].name, "Linux Terminal");
        assert_eq!(
            items[0].body,
            "I want you to act as a \"terminal\".\nReply in code."
        );
        assert_eq!(items[1].body, "Cook simply");
    }

    #[test]
    fn unknown_sources_are_refused() {
        assert!(Target::resolve("evil", None).is_err());
        assert!(Target::resolve("github", Some("x")).is_err());
        assert!(Target::resolve("anthropic", None).is_ok());
    }
}
