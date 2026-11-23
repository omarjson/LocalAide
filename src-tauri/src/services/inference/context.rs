use crate::domain::chat::ChatMessage;

/// Approximate token count of a text, close to what common tokenizers give:
/// about four characters per token for English and code, about two for
/// other alphabets (accented Latin, Cyrillic, Arabic, Greek...), and about
/// one per character for Chinese, Japanese and Korean. The chat's context
/// meter uses the same rule (`src/lib/contextUsage.ts`), so both sides agree.
pub fn estimate_text_tokens(text: &str) -> usize {
    let (mut ascii, mut other, mut wide) = (0usize, 0usize, 0usize);
    for c in text.chars() {
        let code = c as u32;
        if code < 0x80 {
            ascii += 1;
        } else if is_wide(code) {
            wide += 1;
        } else {
            other += 1;
        }
    }
    ascii.div_ceil(4) + other.div_ceil(2) + wide
}

fn is_wide(code: u32) -> bool {
    matches!(code,
        0x1100..=0x11FF
        | 0x2E80..=0x9FFF
        | 0xAC00..=0xD7AF
        | 0xF900..=0xFAFF
        | 0xFF00..=0xFFEF
        | 0x20000..=0x2FFFF)
}

pub struct ContextBudget {
    pub max_context_tokens: usize,
    pub generation_tokens: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_context_tokens: 2048,
            generation_tokens: 512,
        }
    }
}

pub struct BudgetResult {
    pub prompt: String,
    pub was_truncated: bool,
    pub dropped_turns: usize,
    pub estimated_tokens: usize,
}

impl ContextBudget {
    pub fn new(max_context: usize, max_gen: usize) -> Self {
        Self {
            max_context_tokens: max_context,
            generation_tokens: max_gen,
        }
    }

    /// Approximate token count. See [`estimate_text_tokens`].
    pub fn estimate_tokens(text: &str) -> usize {
        estimate_text_tokens(text)
    }

    /// Build a bounded prompt from system prompt and chat history.
    /// Preserves system prompt and latest user messages.
    /// Truncates oldest non-system turns first, returning whether truncation occurred.
    pub fn build_prompt(
        &self,
        system_prompt: Option<&str>,
        history: &[ChatMessage],
        latest_message: &str,
    ) -> BudgetResult {
        let budget_available = self
            .max_context_tokens
            .saturating_sub(self.generation_tokens);

        let sys = system_prompt.unwrap_or("You are a helpful local AI assistant.");
        let sys_tokens = Self::estimate_tokens(sys);
        let latest_tokens = Self::estimate_tokens(latest_message);

        let mut remaining_budget = budget_available.saturating_sub(sys_tokens + latest_tokens);

        let mut included_history: Vec<&ChatMessage> = Vec::new();
        let mut dropped_turns = 0;

        // Iterate backwards through history to include most recent messages first
        for msg in history.iter().rev() {
            let msg_tokens = Self::estimate_tokens(&msg.content) + 4; // role prefix + newline
            if msg_tokens <= remaining_budget {
                included_history.push(msg);
                remaining_budget = remaining_budget.saturating_sub(msg_tokens);
            } else {
                dropped_turns += 1;
            }
        }
        included_history.reverse();

        // Assemble the prompt
        let mut prompt = String::new();
        if !sys.trim().is_empty() {
            prompt.push_str(sys);
            prompt.push_str("\n\n");
        }

        for msg in included_history {
            prompt.push_str(&format!("{}: {}\n", msg.role, msg.content));
        }

        prompt.push_str(&format!("user: {}\nassistant:", latest_message));

        let estimated = Self::estimate_tokens(&prompt);
        let was_truncated = dropped_turns > 0;

        BudgetResult {
            prompt,
            was_truncated,
            dropped_turns,
            estimated_tokens: estimated,
        }
    }
}

impl ContextBudget {
    /// Build the chat as a message list, so the server can apply the model's chat template.
    ///
    /// The system prompt and the latest user message always fit. Older turns are kept
    /// newest first until the budget runs out, then dropped. Kept turns stay in order.
    pub fn build_messages(
        &self,
        system_prompt: Option<&str>,
        history: &[crate::domain::chat::ChatMessage],
        latest_message: &str,
        images: &[String],
    ) -> Vec<super::PromptMessage> {
        let budget_available = self
            .max_context_tokens
            .saturating_sub(self.generation_tokens);

        let sys = system_prompt.unwrap_or("You are a helpful local AI assistant.");
        let sys_tokens = Self::estimate_tokens(sys);
        let latest_tokens = Self::estimate_tokens(latest_message);
        let mut remaining = budget_available.saturating_sub(sys_tokens + latest_tokens);

        // At most one turn per history message is kept; reserve once instead
        // of growing the vector as each kept turn is pushed.
        let mut kept: Vec<super::PromptMessage> = Vec::with_capacity(history.len());
        for msg in history.iter().rev() {
            if msg.role != "user" && msg.role != "assistant" {
                continue;
            }
            let cost = Self::estimate_tokens(&msg.content) + 4;
            if cost > remaining {
                break;
            }
            remaining -= cost;
            kept.push(super::PromptMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
                images: Vec::new(),
            });
        }
        kept.reverse();

        let mut out = Vec::with_capacity(kept.len() + 2);
        if !sys.trim().is_empty() {
            out.push(super::PromptMessage {
                role: "system".to_string(),
                content: sys.to_string(),
                images: Vec::new(),
            });
        }
        out.extend(kept);
        out.push(super::PromptMessage {
            role: "user".to_string(),
            content: latest_message.to_string(),
            images: images.to_vec(),
        });
        out
    }
}

/// What changed when a chat switches model mid-conversation. The history and
/// system prompt are kept; only the token budget for the new model's context
/// window is recalculated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetRecalc {
    pub previous_context: Option<usize>,
    pub context: usize,
    pub generation_tokens: usize,
    pub available_tokens: usize,
    pub kept_turns: usize,
    pub dropped_turns: usize,
    /// A short, human-readable note the UI can show. Empty when nothing changed.
    pub note: String,
}

/// Recompute a prompt budget for a newly chosen model without touching the
/// conversation. Pure: no state, no I/O, so it can be tested in isolation and
/// called from either the command layer or the UI binding layer.
pub fn recalc_budget_for_model(
    previous_context: Option<usize>,
    context: usize,
    generation_tokens: usize,
    system_prompt: Option<&str>,
    history: &[ChatMessage],
    latest: &str,
) -> BudgetRecalc {
    let budget = ContextBudget::new(context, generation_tokens);
    let messages = budget.build_messages(system_prompt, history, latest, &[]);
    // The message list is system + kept history + latest. The system prompt
    // defaults to a non-empty one, so subtract those two fixed slots.
    let kept_turns = messages.len().saturating_sub(2);
    let eligible = history
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .count();
    let dropped_turns = eligible.saturating_sub(kept_turns);
    let available_tokens = context.saturating_sub(generation_tokens);
    let note = match previous_context {
        Some(prev) if prev != context => format!(
            "Context budget recalculated for the new model: {prev} → {context} tokens ({available_tokens} available for the prompt, {kept_turns} turns kept, {dropped_turns} trimmed). This conversation was kept."
        ),
        _ => String::new(),
    };
    BudgetRecalc {
        previous_context,
        context,
        generation_tokens,
        available_tokens,
        kept_turns,
        dropped_turns,
        note,
    }
}

/// Prefix for auto-injected project instruction files in the system prompt.
pub const PROJECT_RULES_HEADING: &str = "## Project rules";

/// Collect project instruction files under `roots` and format them as a
/// system-prompt section. Best-effort: unreadable roots simply contribute no
/// rules, and the caller decides whether the module is enabled. Never fails
/// the chat.
pub fn project_rules(roots: &[std::path::PathBuf], max_bytes: usize) -> Option<String> {
    let files = crate::services::context_files::collect(roots, max_bytes).ok()?;
    if files.files.is_empty() || files.text.trim().is_empty() {
        return None;
    }
    let shorten = if files.truncated {
        "\n\n(Project rules were shortened to fit the model's context.)"
    } else {
        ""
    };
    Some(format!(
        "{PROJECT_RULES_HEADING}\n{}{shorten}",
        files.text.trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, chars: usize) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: "x".repeat(chars),
            ..Default::default()
        }
    }

    #[test]
    fn switching_model_recalculates_the_budget_without_dropping_the_conversation() {
        let history = vec![msg("user", 40), msg("assistant", 40)];
        let small = recalc_budget_for_model(
            Some(2048),
            8192,
            512,
            Some("keep this system prompt"),
            &history,
            "new question",
        );
        assert_eq!(small.previous_context, Some(2048));
        assert_eq!(small.context, 8192);
        assert_eq!(small.available_tokens, 7680);
        assert_eq!(small.kept_turns, 2, "both turns survive a larger window");
        assert_eq!(small.dropped_turns, 0);
        assert!(small.note.contains("2048"));
        assert!(small.note.contains("8192"));
        assert!(small.note.contains("kept"));

        // A smaller window may trim turns, but never the system or latest.
        let tight = recalc_budget_for_model(
            Some(8192),
            128,
            64,
            Some("keep this system prompt"),
            &[msg("user", 4000), msg("assistant", 4000)],
            "new question",
        );
        assert!(tight.available_tokens < small.available_tokens);
        assert!(tight.dropped_turns > 0);
        assert!(tight.note.contains("recalculated"));

        // No previous model (or the same window) means no visible change.
        assert!(
            recalc_budget_for_model(None, 8192, 512, None, &history, "q")
                .note
                .is_empty()
        );
        assert!(
            recalc_budget_for_model(Some(8192), 8192, 512, None, &history, "q")
                .note
                .is_empty()
        );
    }

    #[test]
    fn project_rules_are_capped_and_missing_roots_add_nothing() {
        let missing = std::path::PathBuf::from("/no/such/root/for/context/files");
        assert!(project_rules(std::slice::from_ref(&missing), 1024).is_none());
    }
}
