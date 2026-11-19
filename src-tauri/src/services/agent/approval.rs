//! Waits for the user's answer when the model wants to run a tool that
//! changes something (writing a file, saving a note, an MCP tool). The chat
//! stream sends a `tool_approval` event; the window answers through the
//! `chat_tool_decide` command.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// How long a question waits before it counts as "no".
pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    /// Allow this tool for the rest of this chat.
    AllowForChat,
    Deny,
}

impl Decision {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "allow" | "once" => Some(Self::Allow),
            "allow_chat" | "session" => Some(Self::AllowForChat),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct ApprovalHub {
    pending: Mutex<HashMap<String, Sender<Decision>>>,
}

impl ApprovalHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a question. The answer arrives on the returned receiver.
    pub fn ask(&self, id: &str) -> Receiver<Decision> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.to_string(), tx);
        rx
    }

    /// Deliver the user's answer. False when nothing waits for it (the
    /// answer was stopped, or the question was answered already).
    pub fn decide(&self, id: &str, decision: Decision) -> bool {
        let sender = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        sender.is_some_and(|tx| tx.send(decision).is_ok())
    }

    /// Wait for the answer. Stop and the timeout both count as "no".
    pub fn wait(
        &self,
        id: &str,
        rx: &Receiver<Decision>,
        cancel: &AtomicBool,
        limit: Duration,
    ) -> Decision {
        let deadline = Instant::now() + limit;
        let decision = loop {
            if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
                break Decision::Deny;
            }
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(d) => break d,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break Decision::Deny,
            }
        };
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn answers_reach_the_waiting_question() {
        let hub = Arc::new(ApprovalHub::new());
        let rx = hub.ask("a");
        let h = hub.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            assert!(h.decide("a", Decision::AllowForChat));
        });
        let cancel = AtomicBool::new(false);
        assert_eq!(
            hub.wait("a", &rx, &cancel, Duration::from_secs(5)),
            Decision::AllowForChat
        );
        assert!(!hub.decide("a", Decision::Allow), "answered once only");
    }

    #[test]
    fn stop_and_timeout_mean_no() {
        let hub = ApprovalHub::new();
        let rx = hub.ask("b");
        let cancel = AtomicBool::new(true);
        assert_eq!(
            hub.wait("b", &rx, &cancel, Duration::from_secs(5)),
            Decision::Deny
        );
        let rx = hub.ask("c");
        let cancel = AtomicBool::new(false);
        assert_eq!(
            hub.wait("c", &rx, &cancel, Duration::from_millis(10)),
            Decision::Deny
        );
        assert_eq!(Decision::parse("session"), Some(Decision::AllowForChat));
        assert_eq!(Decision::parse("maybe"), None);
    }
}
