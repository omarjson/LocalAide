//! Context budgeting for the chat loop: how big the turns are, and how to
//! shrink them to fit the window.

use serde_json::Value;

use crate::services::inference::openai_stream::ChatTurn;

use super::AgentRun;

/// Tokens an image takes in the prompt, roughly, for budgeting.
const IMAGE_TOKENS: usize = 750;

pub(crate) fn estimate_tokens(turns: &[ChatTurn]) -> usize {
    use crate::services::inference::context::estimate_text_tokens as text_tokens;
    let tokens: usize = turns
        .iter()
        .map(|t| match t {
            ChatTurn::System(s) => text_tokens(s),
            ChatTurn::User { text, images } => text_tokens(text) + images.len() * IMAGE_TOKENS,
            ChatTurn::Assistant { text, tool_calls } => {
                text_tokens(text)
                    + tool_calls
                        .iter()
                        .map(|c| text_tokens(&c.arguments) + text_tokens(&c.name) + 4)
                        .sum::<usize>()
            }
            ChatTurn::Tool { content, .. } => text_tokens(content) + 4,
        })
        .sum();
    tokens + turns.len() * 4
}

/// Estimated tokens of the tool schemas, which ride along with every
/// request and fill the window like any message.
pub(crate) fn schema_tokens(schemas: &[Value]) -> usize {
    if schemas.is_empty() {
        return 0;
    }
    serde_json::to_string(schemas)
        .map(|s| crate::services::inference::context::estimate_text_tokens(&s))
        .unwrap_or(0)
}

/// Keep the conversation inside the context: older tool results are
/// replaced by a short note first, then the long arguments of older tool
/// calls (a file the model already wrote, say), then the oldest history
/// turns go. The system prompt and the newest user message always stay.
pub fn fit_turns(turns: &mut Vec<ChatTurn>, budget: usize) {
    const REMOVED: &str = "[Earlier tool result removed to save space.]";
    // Still valid JSON, so chat templates that parse arguments keep working.
    const ARGS_REMOVED: &str = r#"{"note":"arguments removed to save space"}"#;
    const LONG_ARGS: usize = 1_000;
    let tool_slots: Vec<usize> = turns
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t, ChatTurn::Tool { .. }))
        .map(|(i, _)| i)
        .collect();
    // The newest tool result is the one the model needs most; keep it.
    for &i in &tool_slots[..tool_slots.len().saturating_sub(1)] {
        if estimate_tokens(turns) <= budget {
            return;
        }
        if let ChatTurn::Tool { content, .. } = &mut turns[i] {
            *content = REMOVED.to_string();
        }
    }
    // Long arguments of tool calls before the newest one.
    let call_slots: Vec<usize> = turns
        .iter()
        .enumerate()
        .filter(
            |(_, t)| matches!(t, ChatTurn::Assistant { tool_calls, .. } if !tool_calls.is_empty()),
        )
        .map(|(i, _)| i)
        .collect();
    for &i in &call_slots[..call_slots.len().saturating_sub(1)] {
        if estimate_tokens(turns) <= budget {
            return;
        }
        if let ChatTurn::Assistant { tool_calls, .. } = &mut turns[i] {
            for call in tool_calls
                .iter_mut()
                .filter(|c| c.arguments.len() > LONG_ARGS)
            {
                call.arguments = ARGS_REMOVED.to_string();
            }
        }
    }
    let last_user = turns
        .iter()
        .rposition(|t| matches!(t, ChatTurn::User { .. }))
        .unwrap_or(0);
    let first = usize::from(matches!(turns.first(), Some(ChatTurn::System(_))));
    let mut removable = last_user.saturating_sub(first);
    while estimate_tokens(turns) > budget && removable > 0 {
        turns.remove(first);
        removable -= 1;
    }
}

/// The answer limit for one round: the user's answer length plus room to
/// think, capped by what is left of the context after the turns and the
/// tool schemas.
pub(crate) fn round_limit(run: &AgentRun, turns: &[ChatTurn], tool_tokens: usize) -> i32 {
    let thinking = if run.thinks {
        run.effort.thinking_allowance()
    } else {
        0
    };
    let wanted = run.answer_tokens.max(64) + thinking;
    let left = run
        .context_tokens
        .saturating_sub(estimate_tokens(turns) + tool_tokens + 64)
        .min(i32::MAX as usize) as i32;
    wanted.min(left).max(256)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::services::inference::openai_stream::ToolCall;

    fn user(text: &str) -> ChatTurn {
        ChatTurn::User {
            text: text.into(),
            images: vec![],
        }
    }

    #[test]
    fn old_tool_results_go_first_then_old_turns() {
        let mut turns = vec![
            ChatTurn::System("system".into()),
            user(&"a".repeat(400)),
            ChatTurn::Assistant {
                text: "b".repeat(400),
                tool_calls: vec![],
            },
            user("now"),
            ChatTurn::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    name: "web_search".into(),
                    arguments: "{}".into(),
                }],
            },
            ChatTurn::Tool {
                call_id: "1".into(),
                name: "web_search".into(),
                content: "x".repeat(4000),
            },
            ChatTurn::Tool {
                call_id: "2".into(),
                name: "read_file".into(),
                content: "y".repeat(400),
            },
        ];
        fit_turns(&mut turns, 400);
        assert!(matches!(&turns[0], ChatTurn::System(_)));
        assert!(
            matches!(&turns.last().unwrap(), ChatTurn::Tool { content, .. } if content.len() == 400)
        );
        assert!(turns.iter().any(
            |t| matches!(t, ChatTurn::Tool { content, .. } if content.starts_with("[Earlier"))
        ));
        assert!(
            turns
                .iter()
                .any(|t| matches!(t, ChatTurn::User { text, .. } if text == "now"))
        );
        assert!(estimate_tokens(&turns) <= 400);
    }
}
