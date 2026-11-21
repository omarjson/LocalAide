use serde::{Deserialize, Deserializer, Serialize};

/// Reads a whole number of milliseconds or seconds, also when the window
/// sent it as a fraction (`performance.now()` gives `2214.0000000000146`).
/// One stray fraction must never make a whole chat unsavable.
fn lenient_u64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    let value = Option::<f64>::deserialize(d)?;
    Ok(value
        .filter(|v| v.is_finite())
        .map(|v| v.max(0.0).round() as u64))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, deserialize_with = "lenient_u64")]
    pub ts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<crate::domain::memory::Citation>>,
    /// The answer in order: thinking, tool calls and text. Saved so a chat
    /// opened later still shows what the model did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<MessagePart>>,
    /// Files attached to a user message, with the text the model read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChatAttachment>>,
    /// Which model wrote an answer, for example "Qwen3 4B" or
    /// "gpt-oss-120b · OpenRouter".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The answer failed. Failed answers are not sent back as history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<bool>,
}

/// One piece of an answer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text {
        text: String,
    },
    Reasoning {
        text: String,
        /// How long the model thought, in milliseconds.
        #[serde(default, deserialize_with = "lenient_u64")]
        ms: Option<u64>,
    },
    Tool {
        id: String,
        name: String,
        title: String,
        #[serde(default)]
        detail: String,
        #[serde(default)]
        group: String,
        #[serde(default)]
        arguments: String,
        /// "running", "waiting", "done", "failed" or "denied".
        status: String,
        #[serde(default)]
        summary: String,
        #[serde(default)]
        output: String,
    },
    /// A question the model asked with options (`ask_user`).
    Question {
        id: String,
        question: String,
        #[serde(default)]
        options: Vec<String>,
        #[serde(default)]
        multiple: bool,
        /// What the user picked.
        #[serde(default)]
        answer: Option<String>,
    },
}

/// A file attached to a message. Images travel as data URLs and are not
/// saved with the chat; text files keep their text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatAttachment {
    pub name: String,
    /// "text" or "image".
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub data_url: Option<String>,
    /// Characters of text read from the file.
    #[serde(default)]
    pub chars: usize,
    /// The text was shortened to fit.
    #[serde(default)]
    pub truncated: bool,
    /// Why the file could not be read, when it could not.
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatThread {
    pub id: String,
    pub title: String,
    pub system_prompt: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
    /// The project folder this chat works in. None = a plain chat in the
    /// shared workspace.
    #[serde(default)]
    pub folder: Option<String>,
}

/// The model the picker chose. No provider = the local model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatTargetRequest {
    #[serde(default)]
    pub provider_id: Option<String>,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamRequest {
    pub message: String,
    pub model: String,
    pub system_prompt: Option<String>,
    /// The thread the message belongs to, for per-session usage counts.
    #[serde(default)]
    pub thread_id: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    /// Earlier turns of this conversation, oldest first. Excludes `message`.
    #[serde(default)]
    pub history: Vec<ChatMessage>,
    /// Images attached to `message`, as `data:image/...;base64,` URLs.
    #[serde(default)]
    pub images: Vec<String>,
    /// "standard" (default), "strict", "unrestricted" or "autonomous".
    #[serde(default)]
    pub mode: Option<String>,
    /// Let the model search the user's notes and documents. Default true.
    #[serde(default)]
    pub use_recall: Option<bool>,
    /// Let the model search the web. Default false; the user turns it on,
    /// so no web request happens without their choice.
    #[serde(default)]
    pub use_web: Option<bool>,
    /// Let the model read and (with approval) write workspace files.
    /// Default true.
    #[serde(default)]
    pub use_files: Option<bool>,
    /// Let the model read saved skills. Default true.
    #[serde(default)]
    pub use_skills: Option<bool>,
    /// MCP servers switched on for this chat. None = every enabled server.
    #[serde(default)]
    pub mcp_servers: Option<Vec<String>>,
    /// Tools the user allowed for the rest of this chat.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Let the model run terminal commands in the workspace. Default true;
    /// commands that change something still ask.
    #[serde(default)]
    pub use_shell: Option<bool>,
    /// "ask" (default), "chat" (allowed for this chat) or "auto".
    #[serde(default)]
    pub approval_mode: Option<String>,
    /// The model chosen in the picker. None = the current default.
    #[serde(default)]
    pub target: Option<ChatTargetRequest>,
    /// How hard the model thinks: "auto", "off", "on", "low", "medium",
    /// "high". None = auto.
    #[serde(default)]
    pub effort: Option<String>,
    /// Text files attached to `message`. Their text is added for the model.
    #[serde(default)]
    pub attachments: Vec<ChatAttachment>,
    /// The user's local date and time, for example "Thursday, 19 November
    /// 2026, 14:05 (Europe/Berlin)". Models do not know today's date.
    #[serde(default)]
    pub now: Option<String>,
    /// The project folder this chat works in (picked by the user). None =
    /// the shared workspace.
    #[serde(default)]
    pub folder: Option<String>,
    /// Let the model save and improve skills on its own. Default true.
    #[serde(default)]
    pub learn_skills: Option<bool>,
    /// Let the model hand work to read-only subagents. Default true.
    #[serde(default)]
    pub use_subagents: Option<bool>,
    /// Tell the model it can draw charts, diagrams and mockups. Default true.
    #[serde(default)]
    pub rich_output: Option<bool>,
}

/// The user's answer to a tool approval question.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolDecisionRequest {
    pub approval_id: String,
    /// "allow", "allow_chat" or "deny".
    pub decision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeRequest {
    pub text: String,
    pub max_words: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeResponse {
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadExportRequest {
    pub thread_id: String,
    pub format: String, // "markdown" | "json"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadExportResponse {
    pub content: String,
    pub filename: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_times_from_the_window_still_save() {
        let json = r#"{"role":"assistant","content":"hi","ts":1731542400000.4,
            "parts":[{"type":"reasoning","text":"t","ms":2214.0000000000146}]}"#;
        let m: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(m.ts, Some(1_731_542_400_000));
        assert!(matches!(
            &m.parts.unwrap()[0],
            MessagePart::Reasoning { ms: Some(2214), .. }
        ));
        let none: ChatMessage = serde_json::from_str(r#"{"role":"user","content":"x"}"#).unwrap();
        assert_eq!(none.ts, None);
        let null: ChatMessage =
            serde_json::from_str(r#"{"role":"user","content":"x","ts":null}"#).unwrap();
        assert_eq!(null.ts, None);
    }

    #[test]
    fn questions_save_and_load() {
        let json = r#"{"role":"assistant","content":"","parts":[{"type":"question","id":"c1","question":"Which?","options":["A","B"],"multiple":false,"answer":"A"},
            {"type":"tool","id":"c2","name":"execute_command","title":"Run a command","group":"shell","status":"done"}]}"#;
        let m: ChatMessage = serde_json::from_str(json).unwrap();
        let parts = m.parts.unwrap();
        assert!(matches!(&parts[0], MessagePart::Question { answer: Some(a), .. } if a == "A"));
        let back = serde_json::to_string(&parts[0]).unwrap();
        assert!(back.contains("\"type\":\"question\""));
    }
}
