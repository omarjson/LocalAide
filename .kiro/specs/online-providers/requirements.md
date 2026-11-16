# Requirements: opt-in online providers

> **How this spec was made.** The feature was decided in `.kiro/specs/kiroween-2026/plan.md` §0 ("Online mode: build it in") and built as plan track E on 2026-11-13 (`3c6b034`, hardened in `afede44`). This requirements/design/tasks set was written on 2026-11-15 from the shipped code, so each criterion points at the code that meets it. The open tasks in `tasks.md` are the part still to be built spec-first.

## Introduction

LocalAide runs chat on a local model by default and sends nothing off the machine. Some users want a stronger model than their computer can run. Online providers let them route chat to an OpenAI-compatible service with their own API key, as an explicit choice that never weakens the local default.

## Glossary

- **Provider:** an OpenAI-compatible endpoint (base URL, model name, API key) the user saves in Settings.
- **Active provider:** the provider chat uses when cloud mode is on.
- **Cloud mode:** the switch that routes chat to the active provider. Off by default.
- **Local host:** `localhost`, or an IPv4 or IPv6 loopback address.

## Requirement 1: off by default

**User story:** As a privacy-minded user, I want the app to stay fully local unless I turn cloud mode on, so that installing it never sends my chats anywhere.

1. WHEN the app is installed or the config file has no cloud settings, THE SYSTEM SHALL start with cloud mode off.
2. WHEN the user saves their first provider, THE SYSTEM SHALL mark it active AND SHALL leave cloud mode off.
3. WHILE cloud mode is off, THE SYSTEM SHALL route every chat request to the local engine.
4. WHEN the user loads a local model, THE SYSTEM SHALL switch chat back to the local engine, whatever the previous mode was.

## Requirement 2: provider management

**User story:** As a user, I want to add, edit, delete and pick providers in Settings, so that I can use the service I already pay for.

1. WHEN the user saves a provider, THE SYSTEM SHALL require a non-empty name, base URL and model name, AND SHALL reject the save with a plain-language message naming the missing field.
2. WHEN the user saves a provider whose base URL and model match another saved provider, THE SYSTEM SHALL reject it as a duplicate.
3. WHEN the user opens the provider form, THE SYSTEM SHALL offer presets (base URL, a first model, where to get a key) that prefill the form.
4. WHEN the user deletes the active provider, THE SYSTEM SHALL turn cloud mode off AND SHALL mark the first remaining provider active, so that chat never moves to a different provider silently.
5. WHEN the user turns cloud mode on with no active provider, THE SYSTEM SHALL refuse with a message saying to add one first.

## Requirement 3: transport safety

**User story:** As a user, I want my key and chats sent only to the address I typed, over an encrypted connection, so that a typo or a hostile server cannot capture them.

1. WHEN a base URL uses `https`, THE SYSTEM SHALL accept it.
2. WHEN a base URL uses `http`, THE SYSTEM SHALL accept it only if its host is a local host, as parsed from the URL (so `127.evil.com` and `0.0.0.0` are refused).
3. WHEN a base URL uses any other scheme, or carries a user name or password, THE SYSTEM SHALL reject it.
4. WHEN a provider answers with a redirect, THE SYSTEM SHALL NOT follow it.
5. WHEN the app starts with cloud mode on and the active provider's base URL fails these checks, THE SYSTEM SHALL stay local AND SHALL log a warning.

## Requirement 4: key handling

**User story:** As a user, I want my API key kept out of sight and tied to its site, so that it is not shown, logged, or sent somewhere new.

1. THE SYSTEM SHALL store the API key only in the local config file AND SHALL send it only as a bearer token to the provider's base URL.
2. WHEN the frontend lists providers, THE SYSTEM SHALL return a masked hint (fixed dots plus at most the last four characters) and never the key.
3. WHEN the user edits a provider and changes its address to a different origin (scheme, host or port) without entering a key, THE SYSTEM SHALL refuse the save and ask for the key again.
4. WHERE the new address is plain HTTP on a local host, THE SYSTEM SHALL instead clear the stored key, since local proxies need none.
5. WHEN the user adds a new provider without a key, THE SYSTEM SHALL refuse unless the address is plain HTTP on a local host.
6. THE SYSTEM SHALL NOT make the in-memory provider target printable with debug formatting.

## Requirement 5: streaming chat

**User story:** As a user, I want online answers to stream and stop exactly like local ones, so that switching modes changes nothing else.

1. WHILE cloud mode is on, THE SYSTEM SHALL send the same prompt (context budget, memory citations, images) to the provider's `/chat/completions` endpoint with streaming on.
2. WHEN the user presses Stop, THE SYSTEM SHALL end the stream at the next received line AND SHALL keep the text received so far.
3. WHEN the provider streams an error event, THE SYSTEM SHALL show it as an error instead of a finished answer.
4. WHEN the stream closes without `[DONE]` and the user did not stop it, THE SYSTEM SHALL report the answer as cut short, with the token count received.
5. WHEN the provider returns 401 or 403, THE SYSTEM SHALL say the API key looks wrong.

## Requirement 6: visible mode

**User story:** As a user, I want to see at a glance when chat is online, so that I never send something private by mistake.

1. WHILE cloud mode is on, THE SYSTEM SHALL show the provider and model in the chat header.
2. WHILE cloud mode is on, THE SYSTEM SHALL replace the chat page's "your messages stay here" line with the provider's name.
3. THE Settings switch SHALL state that chat messages go to the provider when on, and that nothing leaves the computer when off.

## Non-functional requirements

- **Persistence:** THE SYSTEM SHALL write the config before switching the engine, so that a failed disk write never leaves the engine in a mode the file disagrees with.
- **Typed boundary:** every command SHALL take a concrete DTO and return `Result<T, AppError>` (ADR-0001).
- **Secret storage:** keys are in plain text in `config.json`. Moving them to the OS keychain is tracked as task 9.
