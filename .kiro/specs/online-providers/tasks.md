# Tasks: opt-in online providers

Requirement ids refer to `requirements.md`. Tasks 1 to 7 shipped on 2026-11-13; the commits are listed. Tasks 8 to 10 are open.

- [x] 1. Add cloud settings to the config
  - `cloud_llm_enabled`, `cloud_providers`, `active_cloud_provider` in `AppConfig`, defaulting to off.
  - `is_local_host` and `validate_base_url` in `config.rs`, with unit tests.
  - _Requirements: R1.1, R3.1, R3.2, R3.3_ · `3c6b034`; loopback parsing fixed in `46ec305` (audit SEC-05)

- [x] 2. Build `RemoteEngine`
  - Streaming `/chat/completions` client with bearer auth and no redirects.
  - SSE parsing with cancel checks, error events and the `[DONE]` check.
  - _Requirements: R3.4, R5.1 to R5.5, R4.6_ · `3c6b034`; hardened in `afede44` (code review: error events, `[DONE]` check, `data:` tolerance, `RemoteTarget` no longer `Debug`)

- [x] 3. Put both engines behind `RouterEngine`
  - Replace the sidecar in `AppState` with the router; `load()` returns to local.
  - _Requirements: R1.3, R1.4_ · `3c6b034`

- [x] 4. Add the cloud commands
  - `cloud_config_get`, `cloud_provider_save`, `cloud_provider_delete`, `cloud_mode_set`; save the config before applying the mode.
  - Duplicate check; deleting the active provider turns cloud mode off.
  - _Requirements: R1.2, R2.1, R2.2, R2.4, R2.5, R4.1, R4.2_ · `3c6b034`, `118e4a5`, `afede44`

- [x] 5. Restore cloud mode at startup
  - Validate the active provider's URL; fall back to local with a warning.
  - _Requirements: R3.5_ · `afede44`

- [x] 6. Settings and chat UI
  - Online providers tab with the switch, the privacy sentence and the add/edit/delete form.
  - Online badge in the chat header and the cloud-mode description.
  - _Requirements: R6.1, R6.2, R6.3_ · `3c6b034`, `afede44`

- [x] 7. Key follows its site, plus presets
  - Changing a provider's origin needs the key again; loopback HTTP clears it; new providers need a key unless they're on loopback HTTP.
  - `cloud_provider_presets` from the 9Router registry, plus Ollama on loopback.
  - _Requirements: R2.3, R4.3, R4.4, R4.5_ · presets in `59794c4`; key-follows-site in `248085f` (audit-2026-11-14 SEC-10)

- [ ] 8. Test `RemoteEngine` over loopback HTTP
  - Start a `std::net::TcpListener` on 127.0.0.1 that serves canned SSE bodies, as the tests in `services/websearch/mod.rs` do.
  - 8.1 A normal stream ending in `[DONE]` returns every token, with `cancelled = false`.
  - 8.2 A stream that closes without `[DONE]` returns the "closed the stream before completion" error with the token count.
  - 8.3 A mid-stream `{"error": …}` event returns an error, not a partial answer.
  - 8.4 A 401 reply produces the "API key looks wrong" hint.
  - 8.5 A 302 reply is not followed: the server counts the requests it gets, and the redirect target receives none.
  - 8.6 Setting the cancel flag after the first token ends generation with `cancelled = true` and no error.
  - _Requirements: R3.4, R5.2, R5.3, R5.4, R5.5_

- [ ] 9. Move API keys to the OS keychain
  - Store keys under a per-provider entry; keep only a reference in `config.json`.
  - Migrate existing plain-text keys on first launch, then remove them from the file.
  - _Requirements: R4.1; the secret storage NFR_

- [ ] 10. Test the router's mode switching
  - `load()` after `set_cloud_target()` returns `EngineMode::Local`.
  - `status()` in cloud mode with no target reports `loaded: false`.
  - _Requirements: R1.3, R1.4_
