//! The tool calls the loop answers itself, without a tool runner:
//! questions for the user, the to-do list, and saving a skill.

use serde_json::Value;

use crate::app::events::{ChatStreamEvent, QuestionItem, QuestionOption, TodoItem};
use crate::app::state::AppState;
use crate::services::inference::openai_stream::ToolCall;

use super::Sink;
use super::tools;

/// The questions of an `ask_user` call: the `questions` list, or the
/// older single `question` with plain `options`.
fn parse_questions(args: &Value) -> Vec<QuestionItem> {
    let clip = |v: Option<&Value>, max: usize| -> String {
        v.and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .chars()
            .take(max)
            .collect()
    };
    let options_of = |v: Option<&Value>| -> Vec<QuestionOption> {
        v.and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|o| match o {
                        Value::String(label) => Some(QuestionOption {
                            label: label.trim().chars().take(120).collect(),
                            description: String::new(),
                            recommended: false,
                        }),
                        Value::Object(_) => Some(QuestionOption {
                            label: clip(o.get("label"), 120),
                            description: clip(o.get("description"), 200),
                            recommended: o
                                .get("recommended")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        }),
                        _ => None,
                    })
                    .filter(|o| !o.label.is_empty())
                    .take(6)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut items: Vec<QuestionItem> = args
        .get("questions")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .take(4)
                .map(|q| QuestionItem {
                    header: clip(q.get("header"), 40),
                    question: clip(q.get("question"), 400),
                    options: options_of(q.get("options")),
                    multiple: q.get("multiple").and_then(Value::as_bool).unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    if items.is_empty() {
        items.push(QuestionItem {
            header: clip(args.get("header"), 40),
            question: clip(args.get("question"), 400),
            options: options_of(args.get("options")),
            multiple: args
                .get("multiple")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    items.retain(|q| !q.question.is_empty() && q.options.len() >= 2);
    items
}

/// Show an `ask_user` question. False when the arguments are unusable.
pub(crate) fn ask_question(call: &ToolCall, sink: &dyn Sink) -> bool {
    let Ok(args) = tools::parse_arguments(&call.arguments) else {
        return false;
    };
    let items = parse_questions(&args);
    let Some(first) = items.first().cloned() else {
        return false;
    };
    sink.emit(ChatStreamEvent::Question {
        id: call.id.clone(),
        question: first.question,
        options: first.options.into_iter().map(|o| o.label).collect(),
        multiple: first.multiple,
        items,
    });
    true
}

/// `update_todos`: show the model's to-do list.
pub(crate) fn update_todos(call: &ToolCall, sink: &dyn Sink) -> String {
    let Ok(args) = tools::parse_arguments(&call.arguments) else {
        return "Error: the to-do list could not be read.".into();
    };
    let items: Vec<TodoItem> = args
        .get("items")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .take(20)
                .filter_map(|item| {
                    let text: String = item
                        .get("text")
                        .and_then(Value::as_str)?
                        .trim()
                        .chars()
                        .take(200)
                        .collect();
                    if text.is_empty() {
                        return None;
                    }
                    let status = match item.get("status").and_then(Value::as_str).unwrap_or("") {
                        "done" | "completed" | "complete" => "done",
                        "in_progress" | "doing" | "active" => "in_progress",
                        _ => "pending",
                    };
                    Some(TodoItem {
                        text,
                        status: status.into(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if items.is_empty() {
        return "Error: send the whole list, each item with text and status.".into();
    }
    let done = items.iter().filter(|i| i.status == "done").count();
    let total = items.len();
    sink.emit(ChatStreamEvent::Todos { items });
    format!(
        "The list is shown to the user ({done} of {total} done). Update it as you finish steps."
    )
}

/// `save_skill`: save or improve a skill at once, with a note and Undo in
/// the chat.
pub(crate) fn save_skill(
    state: &AppState,
    call: &ToolCall,
    source: &str,
    sink: &dyn Sink,
) -> String {
    let args = tools::parse_arguments(&call.arguments).unwrap_or(Value::Null);
    let text = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let (name, description, body) = (text("name"), text("description"), text("instructions"));
    sink.emit(ChatStreamEvent::ToolCall {
        id: call.id.clone(),
        name: "save_skill".into(),
        title: "Learned a skill".into(),
        detail: name.chars().take(120).collect(),
        group: "skills".into(),
        arguments: serde_json::to_string_pretty(&args)
            .unwrap_or_default()
            .chars()
            .take(3000)
            .collect(),
    });
    match state.skills.learn(&name, &description, &body) {
        Ok(learned) => {
            let action = if learned.created {
                "created"
            } else {
                "updated"
            };
            state.tools.record_event(
                &format!("{source}:save_skill"),
                "skills",
                &learned.skill.name,
                "success",
                Some(action),
            );
            sink.emit(ChatStreamEvent::SkillChange {
                id: call.id.clone(),
                change_id: learned.change_id,
                name: learned.skill.name.clone(),
                action: action.into(),
            });
            sink.emit(ChatStreamEvent::ToolResult {
                id: call.id.clone(),
                ok: true,
                summary: if learned.created {
                    "Saved".into()
                } else {
                    "Improved".into()
                },
                output: body.chars().take(4000).collect(),
            });
            format!(
                "{} the skill \"{}\". The user sees a note with Undo.",
                if learned.created { "Saved" } else { "Improved" },
                learned.skill.name
            )
        }
        Err(e) => {
            sink.emit(ChatStreamEvent::ToolResult {
                id: call.id.clone(),
                ok: false,
                summary: e.message.clone(),
                output: String::new(),
            });
            format!("Error: {}", e.message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn questions_come_in_both_forms() {
        let new = serde_json::json!({"questions": [
            {"header": "Clarify", "question": "What did you mean?", "options": [
                {"label": "Random graph", "description": "A network picture"},
                {"label": "Random chart"}
            ]},
            {"question": "Bad one", "options": ["only one"]}
        ]});
        let items = parse_questions(&new);
        assert_eq!(items.len(), 1, "a question with one option is dropped");
        assert_eq!(items[0].header, "Clarify");
        assert_eq!(items[0].options[0].description, "A network picture");
        let old =
            serde_json::json!({"question": "Which?", "options": ["A", "B"], "multiple": true});
        let items = parse_questions(&old);
        assert_eq!(items[0].options.len(), 2);
        assert!(items[0].multiple);
    }

    #[test]
    fn recommended_option_is_carried_not_invented() {
        // The model may mark one option recommended; anything it does not
        // mark stays false, so the UI never invents a recommendation.
        let args = serde_json::json!({"questions": [
            {"question": "Which?", "options": [
                {"label": "First", "recommended": true},
                {"label": "Second"},
                "Third"
            ]}
        ]});
        let items = parse_questions(&args);
        assert_eq!(items.len(), 1);
        assert!(items[0].options[0].recommended);
        assert!(!items[0].options[1].recommended);
        assert!(!items[0].options[2].recommended);
    }
}
