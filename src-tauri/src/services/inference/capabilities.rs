//! What a model can do: think, read images, call tools, and how much text
//! fits in its context. Providers that publish this (OpenRouter's model
//! list, the local server's `/props`) are trusted first; otherwise the model
//! name decides, using the naming habits of the big model families.

use serde::{Deserialize, Serialize};

/// Capabilities shown in the model picker.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    pub reasoning: bool,
    pub vision: bool,
    pub tools: bool,
    /// Context window in tokens, when known.
    pub context: Option<u64>,
    /// Effort levels the picker offers, lowest first. Empty when the model
    /// does not think. "off" is listed when thinking can be switched off.
    pub efforts: Vec<String>,
}

fn has_any(id: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| id.contains(n))
}

/// Guess from the model name. Lowercase ids from any provider work.
pub fn guess_reasoning(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    // Instruct-only variants of thinking families.
    if has_any(
        &id,
        &["-instruct-2507", "non-reasoning", "nothink", "no-think"],
    ) {
        return false;
    }
    has_any(
        &id,
        &[
            "deepseek-r1",
            "deepseek-reasoner",
            "r1-distill",
            "qwq",
            "qwen3",
            "gpt-oss",
            "gpt-5",
            "o1",
            "o3",
            "o4-mini",
            "thinking",
            "reasoning",
            "magistral",
            "phi-4-reasoning",
            "phi-4-mini-reasoning",
            "glm-4.5",
            "glm-4.6",
            "glm-4.7",
            "glm-5",
            "kimi-k2-thinking",
            "kimi-k2.5",
            "minimax-m2",
            "gemini-2.5",
            "gemini-3",
            "claude-3-7",
            "claude-sonnet-4",
            "claude-opus-4",
            "claude-haiku-4",
            "grok-3-mini",
            "grok-4",
            "seed-oss",
            "hunyuan-a13b",
            "exaone-deep",
            "granite-4",
            "nemotron",
            "big-pickle",
        ],
    ) && !has_any(&id, &["gpt-5-chat", "o1-mini-2024"])
}

pub fn guess_vision(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    has_any(
        &id,
        &[
            "vision",
            "-vl",
            "vl-",
            "llava",
            "pixtral",
            "gpt-4o",
            "gpt-4.1",
            "gpt-5",
            "o3",
            "o4-mini",
            "gemini",
            "gemma-3",
            "gemma3",
            "gemma-4",
            "claude",
            "llama-4",
            "llama4",
            "mistral-small-3",
            "mistral-medium",
            "qwen2.5-vl",
            "qwen3-vl",
            "qwen3.5",
            "minicpm-v",
            "moondream",
            "grok-4",
            "kimi-k2.5",
            "glm-4.5v",
            "glm-4.6v",
            "phi-4-multimodal",
            "smolvlm",
            "internvl",
        ],
    )
}

/// Most current chat models call tools. Old base and embedding models do not.
pub fn guess_tools(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    !has_any(
        &id,
        &[
            "embed",
            "rerank",
            "whisper",
            "tts",
            "dall-e",
            "image",
            "moderation",
            "guard",
            "-base",
            "davinci",
            "babbage",
        ],
    )
}

/// Guess a context window from the name, for providers that do not say.
pub fn guess_context(id: &str) -> Option<u64> {
    let id = id.to_ascii_lowercase();
    if has_any(&id, &["gemini"]) {
        return Some(1_048_576);
    }
    if has_any(&id, &["gpt-4.1", "llama-4"]) {
        return Some(1_000_000);
    }
    if has_any(&id, &["gpt-5", "claude"]) {
        return Some(200_000);
    }
    if has_any(
        &id,
        &[
            "gpt-4o",
            "o3",
            "o4-mini",
            "gpt-oss",
            "deepseek",
            "qwen3",
            "llama-3.1",
            "llama-3.3",
            "mistral",
            "kimi",
            "glm",
        ],
    ) {
        return Some(128_000);
    }
    None
}

/// The effort levels offered for a model that thinks, by family. The full
/// ladder is minimal < low < medium < high < xhigh < max < ultra; a family
/// that answers to fewer rungs lists only those.
pub fn effort_levels(id: &str, local: bool) -> Vec<String> {
    let id = id.to_ascii_lowercase();
    let owned = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    if has_any(&id, &["gpt-oss"]) {
        return owned(&["low", "medium", "high"]);
    }
    if has_any(&id, &["gpt-5", "o1", "o3", "o4-mini"]) {
        return owned(&["minimal", "low", "medium", "high"]);
    }
    if has_any(
        &id,
        &["deepseek-r1", "deepseek-reasoner", "qwq", "r1-distill"],
    ) {
        // These always think; there is nothing to choose.
        return Vec::new();
    }
    if has_any(&id, &["grok-4", "grok-5"]) {
        return owned(&["minimal", "low", "medium", "high", "xhigh"]);
    }
    if local {
        // Hybrid local models (Qwen3, GLM, Granite) switch thinking on or off.
        return owned(&["off", "on"]);
    }
    // An unknown online model: offer the whole ladder and let the provider
    // clamp what it does not know.
    owned(&["minimal", "low", "medium", "high", "xhigh", "max", "ultra"])
}

/// Capabilities from the name alone.
pub fn guess(id: &str, local: bool) -> Capabilities {
    let reasoning = guess_reasoning(id);
    Capabilities {
        reasoning,
        vision: guess_vision(id),
        tools: guess_tools(id),
        context: guess_context(id),
        efforts: if reasoning {
            effort_levels(id, local)
        } else {
            Vec::new()
        },
    }
}

/// What the local server says about its loaded model (`GET /props`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LocalProps {
    pub template_thinks: bool,
    pub vision: Option<bool>,
    pub tools: Option<bool>,
    /// Levels the template renders differently, lowest first.
    pub effort_levels: Vec<String>,
    pub context: Option<u64>,
}

/// Read `/props`. Unknown or missing fields stay unknown.
pub fn parse_props(json: &serde_json::Value) -> LocalProps {
    let template = json
        .get("chat_template")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let caps = json.get("chat_template_caps");
    let flag = |key: &str| {
        caps.and_then(|c| c.get(key))
            .and_then(serde_json::Value::as_bool)
    };
    let effort_levels = caps
        .and_then(|c| c.get("reasoning_effort_levels"))
        .or_else(|| json.get("reasoning_effort_levels"))
        .and_then(serde_json::Value::as_array)
        .map(|levels| {
            levels
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    LocalProps {
        template_thinks: template.contains("<think>")
            || template.contains("enable_thinking")
            || template.contains("reasoning_content")
            || template.contains("reasoning_effort")
            || flag("supports_reasoning_effort") == Some(true),
        vision: json
            .get("modalities")
            .and_then(|m| m.get("vision"))
            .and_then(serde_json::Value::as_bool),
        tools: flag("supports_tool_calls").or_else(|| flag("supports_tools")),
        effort_levels,
        context: json
            .get("default_generation_settings")
            .and_then(|s| s.get("n_ctx"))
            .and_then(serde_json::Value::as_u64),
    }
}

/// Merge the local server's report into a name-based guess.
pub fn with_props(mut caps: Capabilities, props: &LocalProps, id: &str) -> Capabilities {
    if props.template_thinks {
        caps.reasoning = true;
        caps.efforts = if props.effort_levels.is_empty() {
            effort_levels(id, true)
        } else {
            let mut levels = vec!["off".to_string()];
            levels.extend(props.effort_levels.iter().filter(|l| *l != "none").cloned());
            levels
        };
    }
    if let Some(vision) = props.vision {
        caps.vision = vision;
    }
    if let Some(tools) = props.tools {
        caps.tools = tools;
    }
    if props.context.is_some() {
        caps.context = props.context;
    }
    caps
}

/// The effort to send for the picker's choice. Local "on" means "let the
/// template think", which is the server default.
pub fn effort_for_request(choice: Option<&str>) -> Option<String> {
    match choice.map(str::trim) {
        None | Some("") | Some("auto") | Some("on") => None,
        Some(other) => Some(other.to_ascii_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn names_reveal_thinking_and_vision() {
        assert!(guess_reasoning("deepseek/deepseek-r1:free"));
        assert!(guess_reasoning("Qwen3-8B-Q4_K_M"));
        assert!(guess_reasoning("openai/gpt-oss-120b"));
        assert!(!guess_reasoning("qwen3-30b-a3b-instruct-2507"));
        assert!(!guess_reasoning("llama-3.3-70b-versatile"));
        assert!(guess_vision("gpt-4o-mini"));
        assert!(guess_vision("qwen2.5-vl-7b"));
        assert!(!guess_vision("llama-3.3-70b"));
        assert!(!guess_tools("text-embedding-3-small"));
        assert!(guess_tools("mistral-small-latest"));
    }

    #[test]
    fn effort_levels_by_family() {
        assert_eq!(
            effort_levels("gpt-oss-20b", true),
            ["low", "medium", "high"]
        );
        assert!(effort_levels("deepseek-reasoner", false).is_empty());
        assert_eq!(effort_levels("qwen3-4b", true), ["off", "on"]);
        assert_eq!(
            effort_levels("grok-4", false),
            ["minimal", "low", "medium", "high", "xhigh"]
        );
        // Unknown online models get the whole ladder, medium in the middle.
        assert_eq!(
            effort_levels("some-router-model", false),
            ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"]
        );
    }

    #[test]
    fn local_props_override_the_guess() {
        let props = parse_props(&json!({
            "chat_template": "{% if enable_thinking %}<think>{% endif %}",
            "chat_template_caps": {"supports_tool_calls": true, "reasoning_effort_levels": ["low", "high"]},
            "modalities": {"vision": true},
            "default_generation_settings": {"n_ctx": 8192}
        }));
        assert!(props.template_thinks);
        let caps = with_props(guess("my-model", true), &props, "my-model");
        assert!(caps.reasoning && caps.vision && caps.tools);
        assert_eq!(caps.efforts, ["off", "low", "high"]);
        assert_eq!(caps.context, Some(8192));
        assert_eq!(effort_for_request(Some("on")), None);
        assert_eq!(effort_for_request(Some("High")), Some("high".into()));
    }
}
