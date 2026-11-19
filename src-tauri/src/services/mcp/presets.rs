//! Starting points for Settings → Tools and MCP. Remote ones work without a
//! key unless `needs_key` says otherwise; local ones need the named tool
//! (Node's `npx` or Python's `uvx`) on this computer.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct McpPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// "http" or "stdio".
    pub kind: &'static str,
    /// The address for "http".
    pub url: &'static str,
    /// The program and its arguments for "stdio".
    pub command: &'static str,
    pub args: &'static [&'static str],
    /// What must be installed, or which key is needed. Empty when nothing.
    pub requirement: &'static str,
    pub needs_key: bool,
    pub homepage: &'static str,
}

const fn http(
    id: &'static str,
    name: &'static str,
    description: &'static str,
    url: &'static str,
    requirement: &'static str,
    needs_key: bool,
    homepage: &'static str,
) -> McpPreset {
    McpPreset {
        id,
        name,
        description,
        kind: "http",
        url,
        command: "",
        args: &[],
        requirement,
        needs_key,
        homepage,
    }
}

const fn stdio(
    id: &'static str,
    name: &'static str,
    description: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    requirement: &'static str,
    homepage: &'static str,
) -> McpPreset {
    McpPreset {
        id,
        name,
        description,
        kind: "stdio",
        url: "",
        command,
        args,
        requirement,
        needs_key: false,
        homepage,
    }
}

pub const PRESETS: &[McpPreset] = &[
    http(
        "context7",
        "Context7",
        "Current documentation and code examples for programming libraries.",
        "https://mcp.context7.com/mcp",
        "Works without a key; a free key raises the limits.",
        false,
        "https://context7.com",
    ),
    http(
        "deepwiki",
        "DeepWiki",
        "Ask questions about public GitHub repositories.",
        "https://mcp.deepwiki.com/mcp",
        "",
        false,
        "https://deepwiki.com",
    ),
    http(
        "exa",
        "Exa search",
        "Web and code search from Exa.",
        "https://mcp.exa.ai/mcp",
        "",
        false,
        "https://exa.ai",
    ),
    http(
        "microsoft-learn",
        "Microsoft Learn",
        "Search Microsoft's official documentation.",
        "https://learn.microsoft.com/api/mcp",
        "",
        false,
        "https://learn.microsoft.com",
    ),
    http(
        "huggingface",
        "Hugging Face",
        "Search models, datasets and papers on Hugging Face.",
        "https://huggingface.co/mcp",
        "Add a header Authorization: Bearer <your token> for more tools.",
        false,
        "https://huggingface.co/settings/mcp",
    ),
    http(
        "github",
        "GitHub",
        "Issues, pull requests and code on GitHub.",
        "https://api.githubcopilot.com/mcp/",
        "Needs a header Authorization: Bearer <a GitHub token>.",
        true,
        "https://github.com/github/github-mcp-server",
    ),
    stdio(
        "filesystem",
        "Files on this computer",
        "Read and edit files in folders you list. Add the folders at the end of the arguments.",
        "npx",
        &["-y", "@modelcontextprotocol/server-filesystem"],
        "Needs Node.js (npx).",
        "https://github.com/modelcontextprotocol/servers",
    ),
    stdio(
        "memory-graph",
        "Knowledge graph memory",
        "Remembers facts as a small graph between chats.",
        "npx",
        &["-y", "@modelcontextprotocol/server-memory"],
        "Needs Node.js (npx).",
        "https://github.com/modelcontextprotocol/servers",
    ),
    stdio(
        "sequential-thinking",
        "Sequential thinking",
        "Helps the model work through a problem step by step.",
        "npx",
        &["-y", "@modelcontextprotocol/server-sequential-thinking"],
        "Needs Node.js (npx).",
        "https://github.com/modelcontextprotocol/servers",
    ),
    stdio(
        "fetch",
        "Fetch",
        "Reads web pages and turns them into text.",
        "uvx",
        &["mcp-server-fetch"],
        "Needs Python's uv (uvx).",
        "https://github.com/modelcontextprotocol/servers",
    ),
    stdio(
        "git",
        "Git",
        "Reads the history and changes of a Git repository. Add --repository <folder>.",
        "uvx",
        &["mcp-server-git"],
        "Needs Python's uv (uvx).",
        "https://github.com/modelcontextprotocol/servers",
    ),
    stdio(
        "time",
        "Time",
        "Current time and time zone conversion.",
        "uvx",
        &["mcp-server-time"],
        "Needs Python's uv (uvx).",
        "https://github.com/modelcontextprotocol/servers",
    ),
    stdio(
        "playwright",
        "Playwright browser",
        "Opens a browser and clicks, types and reads pages for you.",
        "npx",
        &["-y", "@playwright/mcp@latest"],
        "Needs Node.js (npx). Downloads a browser the first time.",
        "https://github.com/microsoft/playwright-mcp",
    ),
];
