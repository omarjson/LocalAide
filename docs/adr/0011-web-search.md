# ADR-0011: Web Search and Page Reading

Date: 2026-11-14. Status: accepted.

## Context

LocalAide is local-first: the scope contract says network access happens only
for explicit user actions. The operator asked for web search inside the app —
via Exa and Firecrawl, plus any solid keyless search/crawl API — with a place
to paste API keys, and for the online-provider list to draw on the open
9Router registry (`github.com/decolua/9router`, MIT) instead of a hand-maintained
list.

## Decision

1. **Explicit actions only.** Web search runs when the user clicks Search in
   the Search screen's Web mode; page reading runs when the user clicks
   "Read page" on a result. Nothing crawls in the background, and web content
   never enters the library index automatically.

2. **Keyless by default.** DuckDuckGo (the `lite` results page, parsed
   leniently — current `result-link`/`result-snippet` classes with the legacy
   names as fallback), Wikipedia's REST search, and the Jina Reader
   (`r.jina.ai`, ~20 req/min IP-throttled) work with no configuration at all.
   A brand-new install can search the web immediately.

3. **Keyed providers stay clearly keyed.** Exa (`x-api-key`, `/search` with
   `contents.text`) and Firecrawl (`Bearer` key, `/v2/search`, `/v2/scrape`)
   refuse to run with `WEB_NO_KEY` until the user pastes a key in
   Settings → Web search. Keys live in `config.json` on this machine only,
   are sent only to their own provider, and are excluded from backup
   archives unless "Include API keys" is checked — the same treatment as
   cloud-provider keys.

4. **Bounded parsing, honest errors.** Every response body is capped at
   2 MB; status 429 maps to `WEB_RATE_LIMITED`, 401/403 to `WEB_AUTH_FAILED`;
   DuckDuckGo's anomaly page is detected and reported as rate limiting. Page
   reads accept only `http`/`https` URLs without embedded credentials.
   Providers are validated against a fixed table; unknown ids are refused.

5. **Provider presets, not promises.** The Settings provider form ships a
   preset list aggregated from the 9Router registry (LLM7, Synthetic,
   api.airforce, Groq, Cerebras, NVIDIA NIM, OpenRouter, Chutes, Mistral,
   plus the user's own local Ollama). Presets prefill the form and link to
   the key page; they are labeled as free tiers that change without notice.
   Loopback proxies may be saved without a key; remote providers may not.

## Consequences

- The privacy contract in `docs/scope.md` gains one explicit network path;
  everything else about the boundary stands.
- Keyless scraping of DuckDuckGo is ToS-prohibited-by-proxy (it is not an
  official API) and its markup can change; the parser accepts both class
  generations and errors honestly rather than returning garbage.
- The 9Router list is a snapshot of a fast-moving ecosystem; dead lanes are
  expected and are the user's key to debug, not a LocalAide guarantee.
- HTTP-level tests for every provider run against loopback servers — the
  same fault-injection style as the HF client tests.
