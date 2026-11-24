//! Chat modes. A mode adds one instruction block to the system prompt and,
//! for Strict, caps the temperature. The table is also sent to the frontend
//! so the mode picker and Developer → "Mode instructions" show exactly the
//! text the model receives.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMode {
    Standard,
    Strict,
    Unrestricted,
    Autonomous,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatModeInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// Added to the system prompt. Empty for Standard.
    pub instruction: &'static str,
}

pub const MODES: &[ChatModeInfo] = &[
    ChatModeInfo {
        id: "standard",
        label: "Standard",
        description: "Balanced answers. Uses your notes and files when they help.",
        instruction: "",
    },
    ChatModeInfo {
        id: "strict",
        label: "Strict",
        description: "Careful and exact. Sticks to what it knows, cites sources, says when it is unsure.",
        instruction: "Strict mode. Be accurate and careful. Use only facts you are confident about or that appear in the provided context, and cite context as [n]. When you are not sure, say so plainly instead of guessing. Do not invent names, numbers, quotes or sources. Keep answers short and precise. Before suggesting any change to files or settings, describe it and ask the user to confirm.",
    },
    ChatModeInfo {
        id: "unrestricted",
        label: "Unrestricted",
        description: "Direct answers without lectures, extra warnings or needless refusals.",
        instruction: "Unrestricted mode. Treat the user as a capable adult. Answer the question that was asked, directly and completely. Do not add moral lectures, disclaimers, warnings or reminders the user did not ask for, and do not refuse ordinary requests because a topic is sensitive, mature or controversial. Give your honest view when asked. Still follow the law and do not help with anything that would seriously harm people.",
    },
    ChatModeInfo {
        id: "autonomous",
        label: "Autonomous",
        description: "Works through the whole task on its own: plans, gathers sources from the web and your files, then answers.",
        instruction: "Autonomous mode. Work on the task on your own until it is done. Do not ask the user follow-up questions; when something is unclear, pick the most reasonable assumption and state it in one line. Start with a short plan as a numbered list, carry it out step by step with your tools where they help, and finish with a short check of what you delivered and anything still open. Cite web results, documents and notes as [n].",
    },
];

impl ChatMode {
    /// Unknown or missing values are Standard, so old frontends keep working.
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim).unwrap_or("") {
            "strict" => Self::Strict,
            "unrestricted" => Self::Unrestricted,
            "autonomous" => Self::Autonomous,
            _ => Self::Standard,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Strict => "strict",
            Self::Unrestricted => "unrestricted",
            Self::Autonomous => "autonomous",
        }
    }

    pub fn instruction(self) -> &'static str {
        MODES
            .iter()
            .find(|m| m.id == self.id())
            .map(|m| m.instruction)
            .unwrap_or("")
    }

    /// The short form of the mode instruction, for the compact prompt:
    /// the same rules, fewer words, so a small local model reaches its
    /// first tokens sooner. Standard has no instruction either way.
    pub fn compact_instruction(self) -> &'static str {
        match self {
            Self::Standard => "",
            Self::Strict => {
                "Strict mode. Use only facts you are confident about or that appear in the context, and cite context as [n]. When unsure, say so instead of guessing. Keep answers short and precise."
            }
            Self::Unrestricted => {
                "Unrestricted mode. Answer directly and completely. No lectures, disclaimers or refusals for ordinary requests."
            }
            Self::Autonomous => {
                "Autonomous mode. Work on your own until done. When unclear, pick the most reasonable assumption and say so in one line. Start with a short plan, then finish with what you delivered and what is still open."
            }
        }
    }

    /// Strict keeps sampling close to the most likely words.
    pub fn temperature(self, requested: f32) -> f32 {
        match self {
            Self::Strict => requested.min(0.3),
            _ => requested,
        }
    }

    /// Autonomous gathers web context on its own when the web is allowed.
    pub fn gathers_web(self) -> bool {
        self == Self::Autonomous
    }

    /// How many rounds of tool calls one answer may take.
    pub fn max_rounds(self) -> usize {
        match self {
            Self::Autonomous => 16,
            Self::Strict => 6,
            _ => 8,
        }
    }
}

/// Which tool groups are on, for the guidance below.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToolGroups {
    pub web: bool,
    pub notes: bool,
    pub files: bool,
    pub skills: bool,
    pub shell: bool,
    pub mcp: bool,
}

/// How the model should use its tools. Small models need this said plainly,
/// or they search for everything or for nothing.
pub fn tool_guidance(groups: ToolGroups, mode: ChatMode) -> String {
    let mut uses = Vec::new();
    if groups.web {
        uses.push("search the web for recent events, prices, versions or facts you are not sure about, and read a page when a snippet is not enough");
    }
    if groups.notes {
        uses.push("search the user's notes and documents first whenever the question is about their own files, notes, documents or earlier work (do not guess or answer from general knowledge), then answer from what you found and name the file you used, and save a note when they ask you to remember something");
    }
    if groups.files {
        uses.push("read and write files in the LocalAide workspace folder when the user asks");
    }
    if groups.skills {
        uses.push("look at the user's saved skills when a task may match one");
    }
    if groups.shell {
        uses.push("run terminal commands in the workspace when the user asks you to run, build or check something, and read the output before you answer");
    }
    if groups.mcp {
        uses.push("use the tools from the user's MCP servers for what they describe");
    }
    if uses.is_empty() {
        return String::new();
    }
    let effort = if mode == ChatMode::Autonomous {
        "Use tools freely to research and check your work before you answer."
    } else {
        "Use a tool only when it helps. Answer directly, without tools, when you already know the answer or the question is about something general."
    };
    format!(
        "You have tools. {effort} You can {}. Cite sources from tool results as [n]. Never make up a tool result; if a tool fails, say so.",
        uses.join("; ")
    )
}

/// The system prompt the model receives: the user's prompt, then the mode
/// instruction, then any retrieved context.
pub fn compose_system_prompt(user: Option<&str>, mode: ChatMode, context: &str) -> Option<String> {
    let parts: Vec<&str> = [user.unwrap_or(""), mode.instruction(), context]
        .into_iter()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

/// The compact system prompt: same shape, shorter mode instruction. Only
/// used for local models when the user turns the compact prompt on
/// (`AppConfig::use_compact_prompt`, default off). The default-off path
/// never calls this, so its prompt is byte-identical to before.
pub fn compose_compact_system_prompt(
    user: Option<&str>,
    mode: ChatMode,
    context: &str,
) -> Option<String> {
    let parts: Vec<&str> = [user.unwrap_or(""), mode.compact_instruction(), context]
        .into_iter()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

/// The short form of the tool guidance: what the tools can do, in one
/// line, without the effort and citation rules. Used with the compact
/// prompt only.
pub fn compact_tool_guidance(groups: ToolGroups, mode: ChatMode) -> String {
    let mut uses = Vec::new();
    if groups.web {
        uses.push("search the web for recent or uncertain facts");
    }
    if groups.notes {
        uses.push("search the user's notes first for questions about their own files");
    }
    if groups.files {
        uses.push("read and write files in the workspace when asked");
    }
    if groups.skills {
        uses.push("look at saved skills when a task may match one");
    }
    if groups.shell {
        uses.push("run terminal commands in the workspace when asked");
    }
    if groups.mcp {
        uses.push("use the MCP servers' tools for what they describe");
    }
    if uses.is_empty() {
        return String::new();
    }
    let effort = if mode == ChatMode::Autonomous {
        "Use tools freely."
    } else {
        "Use a tool only when it helps."
    };
    format!("You have tools. {effort} You can {}.", uses.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_parse_and_fall_back_to_standard() {
        assert_eq!(ChatMode::parse(Some("strict")), ChatMode::Strict);
        assert_eq!(ChatMode::parse(Some(" autonomous ")), ChatMode::Autonomous);
        assert_eq!(ChatMode::parse(Some("jailbreak")), ChatMode::Standard);
        assert_eq!(ChatMode::parse(None), ChatMode::Standard);
        assert!(ChatMode::Standard.instruction().is_empty());
        assert!(ChatMode::Strict.instruction().starts_with("Strict mode"));
    }

    #[test]
    fn strict_caps_temperature() {
        assert_eq!(ChatMode::Strict.temperature(0.9), 0.3);
        assert_eq!(ChatMode::Strict.temperature(0.1), 0.1);
        assert_eq!(ChatMode::Unrestricted.temperature(0.9), 0.9);
    }

    #[test]
    fn system_prompt_joins_the_parts_in_order() {
        assert_eq!(compose_system_prompt(None, ChatMode::Standard, ""), None);
        let p = compose_system_prompt(Some("Be kind."), ChatMode::Strict, "Context: x").unwrap();
        assert!(p.starts_with("Be kind.\n\nStrict mode"));
        assert!(p.ends_with("Context: x"));
    }

    #[test]
    fn compact_prompt_is_shorter_but_keeps_the_shape() {
        // The default-off path never calls the compact composer, so the
        // full prompt is unchanged; the compact one only swaps the mode
        // instruction and keeps user prompt and context in place.
        for mode in [
            ChatMode::Standard,
            ChatMode::Strict,
            ChatMode::Unrestricted,
            ChatMode::Autonomous,
        ] {
            let full = compose_system_prompt(Some("Be kind."), mode, "Context: x");
            let compact = compose_compact_system_prompt(Some("Be kind."), mode, "Context: x");
            match (full, compact) {
                (Some(f), Some(c)) => {
                    assert!(c.starts_with("Be kind.\n\n"), "{c}");
                    assert!(c.ends_with("Context: x"), "{c}");
                    assert!(
                        c.len() <= f.len(),
                        "compact must not be longer than full ({c} vs {f})"
                    );
                }
                (None, None) => {}
                (f, c) => panic!("full {f:?} and compact {c:?} disagree on empty"),
            }
        }
        // Standard has no instruction either way: the two agree exactly.
        assert_eq!(
            compose_system_prompt(Some("Be kind."), ChatMode::Standard, "Context: x"),
            compose_compact_system_prompt(Some("Be kind."), ChatMode::Standard, "Context: x")
        );
        let guidance = compact_tool_guidance(
            ToolGroups {
                web: true,
                files: true,
                ..ToolGroups::default()
            },
            ChatMode::Standard,
        );
        assert!(guidance.contains("search the web"));
        assert!(guidance.contains("read and write files"));
        assert!(
            guidance.len()
                < tool_guidance(
                    ToolGroups {
                        web: true,
                        files: true,
                        ..ToolGroups::default()
                    },
                    ChatMode::Standard
                )
                .len()
        );
        assert!(compact_tool_guidance(ToolGroups::default(), ChatMode::Standard).is_empty());
    }
}
