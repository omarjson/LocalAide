use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Refuse to read more than this much into memory (the UI truncates anyway).
const MAX_READ_BYTES: u64 = 1024 * 1024;

/// Refuse to write payloads larger than this.
const MAX_WRITE_BYTES: usize = 4 * 1024 * 1024;

/// Directories the sandbox will never touch, even when the path resolves into one.
const FORBIDDEN_ROOTS: &[&str] = &[
    "/System",
    "/usr",
    "/bin",
    "/sbin",
    "/etc",
    "/private/etc",
    "/Library",
];

#[derive(Debug, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub ok: bool,
    pub output: String,
    pub error: Option<String>,
    /// Present on read: how much of the file was returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

impl ToolResult {
    fn ok(output: String) -> Self {
        Self {
            ok: true,
            output,
            error: None,
            truncated: None,
        }
    }
    fn err(error: String) -> Self {
        Self {
            ok: false,
            output: String::new(),
            error: Some(error),
            truncated: None,
        }
    }
}

/// Resolve a user-supplied path against the sandbox rules.
///
/// Rejects traversal outside the intended tree and anything landing in a
/// system directory. Returns an absolute path or a human-readable reason.
fn sandbox_path(raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("empty path".into());
    }
    let p = Path::new(raw);
    let mut normalized = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Normal(s) => normalized.push(s),
            Component::CurDir => {}
            Component::ParentDir => {
                // Escape protection: refuse to pop past the start of the path.
                if !normalized.pop() {
                    return Err(format!("path escapes sandbox: {raw}"));
                }
            }
            Component::RootDir => normalized.push("/"),
            Component::Prefix(_) => return Err("windows prefixes are not supported".into()),
        }
    }

    let forbidden = FORBIDDEN_ROOTS
        .iter()
        .any(|root| normalized.starts_with(root));
    if forbidden {
        return Err(format!("path is outside the sandbox: {raw}"));
    }

    if !normalized.exists() {
        return Err(format!("path does not exist: {raw}"));
    }
    Ok(normalized)
}

/// Read a file with a hard size ceiling.
fn read_capped(path: &Path) -> Result<(String, bool), String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_READ_BYTES {
        let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; MAX_READ_BYTES as usize];
        let mut read = 0usize;
        while read < buf.len() {
            let n = f.read(&mut buf[read..]).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            read += n;
        }
        buf.truncate(read);
        return Ok((String::from_utf8_lossy(&buf).into_owned(), true));
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok((String::from_utf8_lossy(&bytes).into_owned(), false))
}

/// Execute a sandboxed tool call.
///
/// Allowlist: `read_file`, `write_file`, `list_dir`, `delete_file`.
pub fn execute_tool(call: ToolCall) -> ToolResult {
    match call.name.as_str() {
        "read_file" => {
            let path = match call.args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return ToolResult::err("missing path".into()),
            };
            match sandbox_path(path) {
                Err(e) => ToolResult::err(e),
                Ok(p) => match read_capped(&p) {
                    Ok((content, truncated)) => {
                        let mut r = ToolResult::ok(content);
                        r.truncated = Some(truncated);
                        r
                    }
                    Err(e) => ToolResult::err(e),
                },
            }
        }
        "write_file" => {
            let path = match call.args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return ToolResult::err("missing path".into()),
            };
            let content = call
                .args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if content.len() > MAX_WRITE_BYTES {
                return ToolResult::err(format!("payload exceeds {MAX_WRITE_BYTES} byte limit"));
            }
            let p = Path::new(path);
            // New writes stay inside an existing directory, never in system roots.
            let parent = p.parent().unwrap_or(Path::new("/"));
            if let Some(parent) = parent.to_str() {
                if let Ok(rel) = sandbox_path(parent) {
                    if !rel.is_dir() {
                        return ToolResult::err(format!("not a directory: {}", rel.display()));
                    }
                } else {
                    return ToolResult::err(format!("directory outside sandbox: {parent}"));
                }
            }
            match fs::write(p, content) {
                Ok(_) => ToolResult::ok(format!("wrote {} bytes to {path}", content.len())),
                Err(e) => ToolResult::err(e.to_string()),
            }
        }
        "list_dir" => {
            let path = match call.args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return ToolResult::err("missing path".into()),
            };
            match sandbox_path(path) {
                Err(e) => ToolResult::err(e),
                Ok(p) => {
                    if !p.is_dir() {
                        return ToolResult::err(format!("not a directory: {}", p.display()));
                    }
                    match fs::read_dir(&p) {
                        Ok(entries) => {
                            let mut names: Vec<String> = Vec::new();
                            for entry in entries.flatten() {
                                let name = entry.file_name().to_string_lossy().into_owned();
                                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                                names.push(if is_dir { format!("{name}/") } else { name });
                            }
                            names.sort();
                            ToolResult::ok(names.join("\n"))
                        }
                        Err(e) => ToolResult::err(e.to_string()),
                    }
                }
            }
        }
        "delete_file" => {
            let path = match call.args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return ToolResult::err("missing path".into()),
            };
            match sandbox_path(path) {
                Err(e) => ToolResult::err(e),
                Ok(p) => {
                    if !p.is_file() {
                        return ToolResult::err(format!("not a file: {}", p.display()));
                    }
                    match fs::remove_file(&p) {
                        Ok(_) => ToolResult::ok(format!("deleted {}", p.display())),
                        Err(e) => ToolResult::err(e.to_string()),
                    }
                }
            }
        }
        other => ToolResult::err(format!("unknown tool: {other}")),
    }
}
