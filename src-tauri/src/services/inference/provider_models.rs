//! Lists the models an OpenAI-compatible provider serves (`GET {base}/models`).
//!
//! Runs only when the user presses "Load models" in Settings. The same rules
//! as chat apply: the address must pass `validate_base_url`, redirects are not
//! followed (so a key cannot be carried to another site), and the response is
//! size-capped.

use std::time::Duration;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app::error::AppError;

/// OpenRouter's full list is a little over 1 MB; leave headroom.
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_MODELS: usize = 2000;

/// One model a provider offers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderModel {
    pub id: String,
    /// A friendlier name, when the provider sends one.
    pub name: Option<String>,
    pub owned_by: Option<String>,
    pub context_length: Option<u64>,
    /// True when the provider marks the model as free to use.
    pub free: bool,
    /// What the provider says the model can do. None = the provider did not
    /// say, and the picker guesses from the name.
    #[serde(default)]
    pub reasoning: Option<bool>,
    #[serde(default)]
    pub vision: Option<bool>,
    #[serde(default)]
    pub tools: Option<bool>,
}

/// Where a provider lists its models. Almost every provider uses
/// `{base}/models`; Pollinations serves its list at the site root.
pub fn models_endpoint(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if let Ok(url) = url::Url::parse(base)
        && url.host_str() == Some("text.pollinations.ai")
    {
        return "https://text.pollinations.ai/models".to_string();
    }
    format!("{base}/models")
}

pub async fn list_models(base_url: &str, api_key: &str) -> Result<Vec<ProviderModel>, AppError> {
    crate::config::validate_base_url(base_url).map_err(AppError::invalid_input)?;
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::internal(e.to_string()))?;

    let mut request = client
        .get(models_endpoint(base_url))
        .header("Accept", "application/json")
        .header("User-Agent", "LocalAide-models");
    if !api_key.trim().is_empty() {
        request = request.bearer_auth(api_key.trim());
    }
    let audit = crate::services::cloud_audit::start(
        &crate::services::cloud_audit::host_of(base_url),
        base_url,
        "",
        "model list",
        "Asked which models it offers (no chat content)",
        0,
        Some(api_key),
    );
    let sent = request.send().await;
    match &sent {
        Ok(r) => audit.finish(
            r.status().is_success(),
            &format!("HTTP {}", r.status().as_u16()),
        ),
        Err(_) => audit.finish(false, "could not connect"),
    }
    let response = sent.map_err(|e| {
        AppError::new(
            "PROVIDER_UNREACHABLE",
            "Could not reach the provider. Check the address and your connection. A router on this computer must be running.",
        )
        .with_detail(e.to_string())
        .retryable()
    })?;

    let status = response.status();
    if status.is_redirection() {
        return Err(AppError::invalid_input(
            "The provider sent LocalAide to another address. Use the final address instead.",
        ));
    }
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(AppError::new(
            "PROVIDER_AUTH",
            if api_key.trim().is_empty() {
                "This provider needs an API key to list its models. Paste your key first."
            } else {
                "The provider did not accept this API key."
            },
        ));
    }
    if status.as_u16() == 404 {
        return Err(AppError::new(
            "PROVIDER_NO_LIST",
            "This provider does not share a model list. Type the model name instead.",
        ));
    }
    if !status.is_success() {
        return Err(AppError::new(
            "PROVIDER_ERROR",
            format!("The provider answered with an error ({status}). Try again later."),
        )
        .retryable());
    }

    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::internal(e.to_string()))?;
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(AppError::invalid_input(
                "The model list is too large to read.",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    let json: Value = serde_json::from_slice(&body).map_err(|_| {
        AppError::new(
            "PROVIDER_NO_LIST",
            "The provider's answer was not a model list. Type the model name instead.",
        )
    })?;
    Ok(parse_models(&json))
}

/// Accepts the OpenAI shape (`{"data": [{"id": …}]}`), a bare array of
/// objects, or a bare array of names.
pub fn parse_models(json: &Value) -> Vec<ProviderModel> {
    let items = match json {
        Value::Array(items) => items.as_slice(),
        Value::Object(map) => match map.get("data").or_else(|| map.get("models")) {
            Some(Value::Array(items)) => items.as_slice(),
            _ => &[],
        },
        _ => &[],
    };

    let mut out: Vec<ProviderModel> = Vec::new();
    for item in items.iter().take(MAX_MODELS) {
        let model = match item {
            Value::String(id) => ProviderModel {
                id: id.clone(),
                name: None,
                owned_by: None,
                context_length: None,
                free: looks_free(id),
                reasoning: None,
                vision: None,
                tools: None,
            },
            Value::Object(map) => {
                let text = |key: &str| map.get(key).and_then(Value::as_str).map(str::to_string);
                let Some(id) = text("id").or_else(|| text("name")) else {
                    continue;
                };
                // Gemini's compatibility layer prefixes ids with "models/".
                let id = id.strip_prefix("models/").unwrap_or(&id).to_string();
                let name = text("name")
                    .or_else(|| text("display_name"))
                    .filter(|n| n != &id);
                let context_length = map
                    .get("context_length")
                    .or_else(|| map.get("context_window"))
                    .or_else(|| map.get("max_context_length"))
                    .or_else(|| {
                        map.get("top_provider")
                            .and_then(|t| t.get("context_length"))
                    })
                    .and_then(Value::as_u64);
                // OpenRouter and gateways built on it list the request
                // fields a model accepts and its input kinds.
                let params: Option<Vec<&str>> = map
                    .get("supported_parameters")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).collect());
                let reasoning = params
                    .as_ref()
                    .map(|p| {
                        p.iter()
                            .any(|x| *x == "reasoning" || *x == "include_reasoning")
                    })
                    .or_else(|| map.get("reasoning").and_then(Value::as_bool));
                let tools = params
                    .as_ref()
                    .map(|p| p.contains(&"tools"))
                    .or_else(|| map.get("tool_call").and_then(Value::as_bool));
                let vision = map
                    .get("architecture")
                    .and_then(|a| a.get("input_modalities"))
                    .or_else(|| map.get("input_modalities"))
                    .and_then(Value::as_array)
                    .map(|m| m.iter().any(|x| x.as_str() == Some("image")))
                    .or_else(|| map.get("vision").and_then(Value::as_bool));
                let priced_free = map.get("pricing").is_some_and(|p| {
                    let zero = |key: &str| {
                        p.get(key)
                            .map(|v| match v {
                                Value::String(s) => s.parse::<f64>().is_ok_and(|n| n == 0.0),
                                Value::Number(n) => n.as_f64() == Some(0.0),
                                _ => false,
                            })
                            .unwrap_or(false)
                    };
                    zero("prompt") && zero("completion")
                });
                ProviderModel {
                    free: looks_free(&id) || priced_free,
                    id,
                    name,
                    owned_by: text("owned_by"),
                    context_length,
                    reasoning,
                    vision,
                    tools,
                }
            }
            _ => continue,
        };
        if !model.id.trim().is_empty() && !out.iter().any(|m| m.id == model.id) {
            out.push(model);
        }
    }
    out
}

/// Free models are marked in their names: ":free" (OpenRouter, Kilo),
/// "-free" (OpenCode Zen) or "/free" (Kilo's auto router).
fn looks_free(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    id.ends_with(":free") || id.ends_with("-free") || id.ends_with("/free") || id == "big-pickle"
}

#[cfg(test)]
mod tests {
    use super::{models_endpoint, parse_models};
    use serde_json::json;

    #[test]
    fn parses_the_openai_shape() {
        let list = parse_models(&json!({
            "object": "list",
            "data": [
                {"id": "gpt-oss-120b", "owned_by": "openai"},
                {"id": "meta-llama/llama-3.3-70b-instruct:free", "context_length": 131072},
                {"id": "gpt-oss-120b"}
            ]
        }));
        assert_eq!(list.len(), 2, "duplicates are dropped");
        assert_eq!(list[0].owned_by.as_deref(), Some("openai"));
        assert!(!list[0].free);
        assert!(list[1].free);
        assert_eq!(list[1].context_length, Some(131072));
    }

    #[test]
    fn openrouter_zero_pricing_is_free() {
        let list = parse_models(&json!({"data": [
            {"id": "a/b", "name": "A B", "pricing": {"prompt": "0", "completion": "0"}},
            {"id": "c/d", "pricing": {"prompt": "0.000001", "completion": "0"}}
        ]}));
        assert!(list[0].free);
        assert_eq!(list[0].name.as_deref(), Some("A B"));
        assert!(!list[1].free);
    }

    #[test]
    fn openrouter_capabilities_are_read() {
        let list = parse_models(&json!({"data": [{
            "id": "a/b",
            "supported_parameters": ["tools", "reasoning", "max_tokens"],
            "architecture": {"input_modalities": ["text", "image"]},
            "top_provider": {"context_length": 65536}
        }, {"id": "c/d"}]}));
        assert_eq!(list[0].reasoning, Some(true));
        assert_eq!(list[0].tools, Some(true));
        assert_eq!(list[0].vision, Some(true));
        assert_eq!(list[0].context_length, Some(65536));
        assert_eq!(list[1].reasoning, None);
    }

    #[test]
    fn gemini_prefixes_and_bare_arrays() {
        let list = parse_models(&json!({"data": [{"id": "models/gemini-flash-latest"}]}));
        assert_eq!(list[0].id, "gemini-flash-latest");
        let list = parse_models(&json!(["openai", {"name": "mistral"}]));
        assert_eq!(
            list.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["openai", "mistral"]
        );
        assert!(parse_models(&json!({"error": "nope"})).is_empty());
    }

    #[test]
    fn free_names_are_recognised() {
        let list = parse_models(&json!({"data": [
            {"id": "kilo-auto/free"}, {"id": "glm-4.7-free"}, {"id": "big-pickle"}, {"id": "claude-sonnet"}
        ]}));
        let free: Vec<bool> = list.iter().map(|m| m.free).collect();
        assert_eq!(free, [true, true, true, false]);
    }

    #[test]
    fn model_list_addresses() {
        assert_eq!(
            models_endpoint("https://openrouter.ai/api/v1/"),
            "https://openrouter.ai/api/v1/models"
        );
        assert_eq!(
            models_endpoint("http://localhost:20128/v1"),
            "http://localhost:20128/v1/models"
        );
        assert_eq!(
            models_endpoint("https://text.pollinations.ai/v1"),
            "https://text.pollinations.ai/models"
        );
    }
}
