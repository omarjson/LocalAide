use crate::domain::chat::ChatMessage;

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

    /// Approximate token count (roughly 4 characters per token).
    pub fn estimate_tokens(text: &str) -> usize {
        (text.chars().count() + 3) / 4
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
        let budget_available = self.max_context_tokens.saturating_sub(self.generation_tokens);

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
