//! Short, tool-less requests to the chat's model: improving a prompt and
//! suggesting the next one. They use the same model as the chat (local, or
//! the online provider the user picked, which the cloud audit log records).

use std::sync::atomic::AtomicBool;

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::inference::openai_stream::{
    ChatTurn, Delta, Effort, initial_options, stream_with_retries,
};
use crate::services::inference::router::ChatTarget;

/// One reply, without thinking, cut at `max_tokens`.
pub fn complete(
    state: &AppState,
    target: &ChatTarget,
    system: &str,
    user: &str,
    max_tokens: i32,
) -> Result<String, AppError> {
    let engine = state.inference_engine.clone();
    let endpoint = engine.endpoint_for(target)?;
    let mut turns = vec![
        ChatTurn::System(system.to_string()),
        ChatTurn::User {
            text: user.to_string(),
            images: Vec::new(),
        },
    ];
    let mut opts = initial_options(endpoint.flavor, max_tokens, 0.4, Vec::new(), Effort::Off);
    let never = AtomicBool::new(false);
    let mut text = String::new();
    let mut thinking = String::new();
    let (outcome, _) = stream_with_retries(
        engine.chat_client(),
        &endpoint,
        &mut turns,
        &mut opts,
        &never,
        &mut |delta| match delta {
            Delta::Content(t) => text.push_str(&t),
            Delta::Reasoning(t) => thinking.push_str(&t),
        },
        &mut |_| {},
    )?;
    let reply = if text.trim().is_empty() {
        outcome.content
    } else {
        text
    };
    Ok(clean(&reply))
}

/// Strip what small models wrap around a short answer: thinking tags,
/// quotes, a leading label.
pub fn clean(reply: &str) -> String {
    let mut text = reply.trim().to_string();
    if let Some(end) = text.find("</think>") {
        text = text[end + "</think>".len()..].trim().to_string();
    }
    for label in [
        "Improved prompt:",
        "Prompt:",
        "Suggestion:",
        "Next prompt:",
        "Here is the improved prompt:",
    ] {
        if text.len() >= label.len() && text[..label.len()].eq_ignore_ascii_case(label) {
            text = text[label.len()..].trim().to_string();
        }
    }
    let quotes: &[char] = &['"', '“', '”', '\'', '`'];
    text.trim_matches(quotes).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn wrappers_are_removed() {
        assert_eq!(
            clean("<think>hmm</think>\n\"Plan my week\""),
            "Plan my week"
        );
        assert_eq!(clean("Improved prompt: Write a haiku"), "Write a haiku");
        assert_eq!(clean("  “Compare A and B”  "), "Compare A and B");
    }
}
