//! Provider failover: what to try after the current target fails with a
//! retryable error, before the first token of a round.

use std::collections::HashSet;

use crate::app::state::AppState;
use crate::services::inference::openai_stream::Endpoint;
use crate::services::inference::router::{ChatTarget, RouterEngine};

/// One step of the failover chain: another provider, or the local model.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FailoverCandidate {
    Provider(String),
    Local,
}

/// The order to try after the current target fails with a retryable error:
/// the user's provider order minus the current and already-tried providers,
/// then the local model last. Pure, so it is unit-tested below.
fn failover_chain(
    provider_order: &[String],
    current_id: Option<&str>,
    tried: &HashSet<String>,
) -> Vec<FailoverCandidate> {
    let mut chain: Vec<FailoverCandidate> = provider_order
        .iter()
        .filter(|id| Some(id.as_str()) != current_id && !tried.contains(*id))
        .map(|id| FailoverCandidate::Provider(id.clone()))
        .collect();
    if !tried.contains("local") {
        chain.push(FailoverCandidate::Local);
    }
    chain
}

/// Compare provider addresses ignoring a trailing slash.
fn same_base_url(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// Try the next failover target after a retryable error. Returns the new
/// target and its endpoint, or None when the chain is exhausted.
///
/// Fail-open by design: any problem here (config unreadable, endpoint
/// unbuildable, no model loaded locally) returns None and the caller
/// reports the original error unchanged. A local chat never fails over to
/// a cloud provider — that would send the conversation somewhere the user
/// did not choose.
pub(crate) fn next_failover_target(
    state: &AppState,
    engine: &RouterEngine,
    current: &ChatTarget,
    tried: &mut HashSet<String>,
) -> Option<(ChatTarget, Endpoint)> {
    let ChatTarget::Remote(remote) = current else {
        return None;
    };
    let Ok(cfg) = crate::commands::cloud::load_config(state) else {
        return None;
    };
    let current_id = cfg
        .cloud_providers
        .iter()
        .find(|p| same_base_url(&p.base_url, &remote.base_url))
        .map(|p| p.id.clone());
    if let Some(id) = &current_id {
        tried.insert(id.clone());
    }
    let order = cfg.provider_order();
    for candidate in failover_chain(&order, current_id.as_deref(), tried) {
        match candidate {
            FailoverCandidate::Provider(id) => {
                let Some(p) = cfg.cloud_providers.iter().find(|p| p.id == id && p.enabled) else {
                    continue;
                };
                tried.insert(id.clone());
                let target = ChatTarget::Remote(crate::commands::cloud::remote_target(p, None));
                // No key override: the target carries its own stored key
                // (the router's key pool belongs to the active provider).
                if let Ok(endpoint) = engine.endpoint_for_with_key(&target, None) {
                    return Some((target, endpoint));
                }
            }
            FailoverCandidate::Local => {
                tried.insert("local".to_string());
                if let Ok(endpoint) = engine.endpoint_for(&ChatTarget::Local) {
                    return Some((ChatTarget::Local, endpoint));
                }
                return None;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failover_chain_skips_current_and_tried_and_ends_local() {
        let order = ["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(
            failover_chain(&order, Some("a"), &HashSet::new()),
            vec![
                FailoverCandidate::Provider("b".into()),
                FailoverCandidate::Provider("c".into()),
                FailoverCandidate::Local,
            ]
        );
        let mut tried = HashSet::new();
        tried.insert("b".to_string());
        assert_eq!(
            failover_chain(&order, Some("a"), &tried),
            vec![
                FailoverCandidate::Provider("c".into()),
                FailoverCandidate::Local,
            ]
        );
        let mut tried = HashSet::new();
        tried.insert("b".to_string());
        tried.insert("c".to_string());
        tried.insert("local".to_string());
        assert!(failover_chain(&order, Some("a"), &tried).is_empty());
    }

    #[test]
    fn failover_chain_with_no_current_still_ends_local() {
        let order = ["a".to_string()];
        assert_eq!(
            failover_chain(&order, None, &HashSet::new()),
            vec![
                FailoverCandidate::Provider("a".into()),
                FailoverCandidate::Local,
            ]
        );
    }

    #[test]
    fn failover_chain_keeps_the_users_order() {
        let order = ["groq".to_string(), "openrouter".to_string()];
        assert_eq!(
            failover_chain(&order, Some("openrouter"), &HashSet::new()),
            vec![
                FailoverCandidate::Provider("groq".into()),
                FailoverCandidate::Local,
            ]
        );
    }
}
