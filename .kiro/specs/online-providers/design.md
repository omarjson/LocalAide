# Design: opt-in online providers

See `requirements.md` for the criteria (R1.1 to R6.3) this design meets.

## Overview

Chat already talked to one `InferenceEngine` trait (`services/inference/mod.rs`). Online mode adds a second engine and a router in front of both, so the chat command, context budget, memory citations and cancellation stay the same in either mode. This is the "stitched" part of LocalAide: a C++ llama-server child process and any cloud API sit behind one Rust trait, and the Svelte UI cannot tell them apart.

```
ChatScreen.svelte ──invoke──▶ commands/chat.rs (chat_stream)
                                   │
                                   ▼
                       RouterEngine (InferenceEngine)
                        │ mode = Local  │ mode = Cloud
                        ▼               ▼
                 SidecarEngine     RemoteEngine ──HTTPS──▶ provider /chat/completions
                 (llama-server     (reqwest, no redirects,
                  on 127.0.0.1)     bearer key)

SettingsScreen.svelte ──invoke──▶ commands/cloud.rs ──▶ config.json (atomic save)
                                        └──▶ apply_engine_mode() → RouterEngine
```

## Components

### `RouterEngine` (`services/inference/router.rs`)

- Holds a `SidecarEngine`, a `RemoteEngine` and an `EngineMode` (`Local` or `Cloud`, default `Local`).
- `generate()` forwards to whichever engine the mode selects (R1.3).
- `load()` calls `set_local()` first, so loading a model always returns to local (R1.4).
- `set_cloud_target()` keeps the local model loaded, so switching back is instant and an answer in progress is not killed.
- `status()` reports `"{model} ({label})"` and a fixed 8,192-token context in cloud mode.

### `RemoteEngine` (`services/inference/remote.rs`)

- Blocking `reqwest` client: 15 s connect timeout, 60-minute total timeout, `redirect::Policy::none()` (R3.4).
- POSTs `{model, messages, max_tokens, temperature, stream: true}` to `{base_url}/chat/completions`, reusing the sidecar's `wire_message` so images and roles serialize the same way (R5.1).
- Reads SSE lines. It checks the cancel flag before each line (R5.2), accepts `data:` with or without a space, fails on an `error` event (R5.3), and fails when the stream ends without `[DONE]` unless the user cancelled (R5.4).
- `RemoteTarget` is `Clone + PartialEq` but not `Debug` (R4.6).

### Commands (`commands/cloud.rs`)

| Command | Request | Effect |
|---|---|---|
| `cloud_config_get` | none | Returns `CloudConfigStatus` with masked keys (R4.2) |
| `cloud_provider_save` | `CloudProviderSaveRequest` | Validates (R2.1, R2.2, R3.1 to R3.3), applies the key rules (R4.3 to R4.5), makes the first provider active (R1.2) |
| `cloud_provider_delete` | `provider_id` | Turns cloud mode off if the active provider goes (R2.4) |
| `cloud_mode_set` | `CloudModeRequest` | Switches mode, optionally moves the active provider (R2.5) |
| `cloud_provider_presets` | none | Static list from the 9Router registry plus Ollama on loopback (R2.3) |

Every mutating command loads the config, changes it, **saves it, then** calls `apply_engine_mode()` (persistence NFR).

### Validation (`config.rs`)

- `is_local_host(&Url)` matches the parsed host: the domain `localhost`, or an IPv4/IPv6 address where `is_loopback()` is true (R3.2).
- `validate_base_url()` rejects URLs with credentials, allows `https`, and allows `http` only for local hosts (R3.1 to R3.3).
- `same_origin()` in `commands/cloud.rs` compares scheme, host and port for the key-follows-site rule (R4.3).

### Startup (`lib.rs`)

If `cloud_llm_enabled` is set and the active provider passes `validate_base_url`, the router starts in cloud mode. Otherwise it stays local and the saved local model loads in the background (R3.5).

### Frontend

- `src/lib/api/client.ts`: `getCloudConfig`, `saveCloudProvider`, `deleteCloudProvider`, `setCloudMode`, `cloudProviderPresets`. Types in `bindings.ts`.
- `SettingsScreen.svelte`, "Online providers" tab: the switch with its privacy sentence (R6.3), a preset picker, and an add/edit form. On edit, the key field starts empty, which means "keep the stored key".
- `ChatScreen.svelte`: reads the cloud config. While cloud mode is on it shows `Online: {model} · {label}` in amber in the header, and changes the page description (R6.1, R6.2).

## Data model

`AppConfig` in `config.rs` gains:

```rust
pub cloud_llm_enabled: bool,              // default false (R1.1)
pub cloud_providers: Vec<CloudProvider>,  // { id, label, base_url, model, api_key }
pub active_cloud_provider: Option<String>,
```

Provider ids are `cp-` plus 32 hex characters from `getrandom`.

## Error handling

All failures are `AppError` with plain-language text:

- **Validation:** `invalid_input` with the field and the fix ("Give the provider a name.", "Use HTTPS for any remote provider.").
- **Network:** "Could not reach {label}: {error}. Check the base URL and your connection."
- **HTTP status:** "{label} returned {code}:", with a key hint on 401 or 403 (R5.5).
- **Stream problems:** mid-stream error events and early close become errors, never truncated "finished" answers.

## Alternatives considered

- **Separate cloud chat command.** Rejected: it would duplicate the context budget, citations and cancellation, and the two paths would drift.
- **Keys in the OS keychain.** Deferred (ADR-0006 amendment). It needs a platform crate and a migration; the plain config file shipped first, with masking in the UI (task 9).
- **Follow same-origin redirects.** Rejected for now. No preset needs redirects, and refusing them all is simpler to reason about.

## Testing strategy

- **Unit tests, existing:** `config.rs` (`config_round_trips_cloud_providers`, `plain_http_is_loopback_only`, `loopback_check_uses_the_parsed_host`) and `commands/cloud.rs` (`a_key_stays_with_its_site`, `provider_ids_are_random`).
- **Gap:** `RemoteEngine`'s SSE handling (R5.2 to R5.5) has no test. Task 8 adds loopback HTTP tests, using the `TcpListener` pattern from the tests in `services/websearch/mod.rs`.
