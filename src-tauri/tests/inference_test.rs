use app_lib::domain::chat::ChatMessage;
use app_lib::services::inference::context::ContextBudget;
use app_lib::services::inference::prompt::{PromptBuilder, PromptFormat};

#[test]
fn test_context_budget_within_limits() {
    let budget = ContextBudget::new(2048, 512); // 1536 tokens available
    let history = vec![
        ChatMessage {
            role: "user".to_string(),
            content: "Hello assistant".to_string(),
            ts: Some(100),
            citations: None,
        },
        ChatMessage {
            role: "assistant".to_string(),
            content: "Hello! How can I help you today?".to_string(),
            ts: Some(101),
            citations: None,
        },
    ];

    let result = budget.build_prompt(
        Some("You are a helpful assistant."),
        &history,
        "What is the capital of France?",
    );

    assert!(!result.was_truncated);
    assert_eq!(result.dropped_turns, 0);
    assert!(result.prompt.contains("You are a helpful assistant."));
    assert!(result.prompt.contains("Hello assistant"));
    assert!(result.prompt.contains("What is the capital of France?"));
}

#[test]
fn test_context_budget_truncates_oldest_turns() {
    // Very tight budget: 100 context, 40 gen = 60 tokens available (~240 chars)
    let budget = ContextBudget::new(100, 40);

    let long_old_msg = "A".repeat(160); // ~40 tokens
    let medium_msg = "B".repeat(80); // ~20 tokens

    let history = vec![
        ChatMessage {
            role: "user".to_string(),
            content: long_old_msg, // Oldest turn, should be dropped
            ts: Some(1),
            citations: None,
        },
        ChatMessage {
            role: "assistant".to_string(),
            content: medium_msg, // More recent turn
            ts: Some(2),
            citations: None,
        },
    ];

    let result = budget.build_prompt(Some("System prompt"), &history, "Latest question");

    // Oldest turn must be dropped, not latest message or system prompt
    assert!(result.was_truncated);
    assert!(result.dropped_turns >= 1);
    assert!(result.prompt.contains("System prompt"));
    assert!(result.prompt.contains("Latest question"));
    assert!(!result.prompt.contains(&"A".repeat(160)));
}

#[test]
fn test_prompt_builder_rag_citations() {
    let builder = PromptBuilder::new(PromptFormat::Standard);
    let base_prompt = "user: What is our vacation policy?\nassistant:";
    let citations = [
        (
            "handbook.md",
            "Employees receive 20 days of paid time off per calendar year.",
        ),
        ("benefits.md", "Sick leave is separate from PTO."),
    ];

    let augmented = builder.append_rag_context(base_prompt, &citations);
    assert!(augmented.contains("Context from library:"));
    assert!(augmented.contains("[1] (Source: handbook.md):"));
    assert!(augmented.contains("Employees receive 20 days"));
    assert!(augmented.contains("[2] (Source: benefits.md):"));
    assert!(augmented.contains("user: What is our vacation policy?"));
}
