# LocalAide 0.2: install, trust and polish

Status: implemented on branch `release/0.2-polish`; waiting for CI and for a test on real machines. Written 2026-11-18 from Omar's test of v0.1.1 on an Intel (x64) Mac and on Windows 11.

## What the test found

| # | Finding | Cause in the code |
|---|---|---|
| 1 | Updating means uninstalling the old version and installing the new one. | No in-app updater. `docs/release.md` lists it as future work (ADR-0005). |
| 2 | No privacy policy, terms of use or other legal pages. | None exist in the repo, the installer or the app. |
| 3 | The installer and app do not show the LocalAide logo. | `src-tauri/icons/*` are still the default Tauri icons (teal and yellow rings) from the first commit. The NSIS installer has no header, sidebar or license page. The DMG has no background. |
| 4 | Search boxes and selects show stray shapes inside them. | `main.css` forces `:focus-visible { outline … !important }` on bare inputs that sit inside bordered wrappers, so a second white box is drawn inside the field. `type="search"` shows the native clear "x" next to the custom one. WebView2 adds its own password-reveal and clear buttons. The one native `<select>` is unstyled. |
| 5 | The app feels empty and has little motion. | Screens fade in only, lists and tabs don't animate, the setup wizard and settings sections switch without transitions, the command palette only lists screens. |
| 6 | Skills cannot be found online. | Skills are local saved prompts only. |
| 7 | Online providers are thin: no keyless free models, no 9Router or OmniRoute, no model list. | A key is required for every non-loopback provider. The model is free text. There is no `GET /models` call. |
| 8 | No way to see trending or uncensored Hugging Face models. | `hf_search` always sorts by downloads and filters only `gguf`. |

## Plan

### A. Install and update
1. Regenerate every icon from `public/logo.svg` (`tauri icon`): `.ico`, `.icns`, PNGs, Windows Store tiles.
2. NSIS installer: installer icon, branded header and sidebar images, the Terms of Use as a license page, per-user install so updates need no admin prompt.
3. DMG: branded background and icon positions.
4. In-app updater with `tauri-plugin-updater` and `tauri-plugin-process`.
   - The update public key is compiled in from `LOCALAIDE_UPDATER_PUBKEY` at build time. Builds without it keep today's "open the download page" check, so nothing breaks before the owner creates the key.
   - Nothing runs at startup. The user presses "Check for updates", sees the version and notes, presses "Install and restart".
   - CI signs updater artifacts and writes `latest.json` only when `TAURI_SIGNING_PRIVATE_KEY` is set.
5. Owner step (cannot be done by the agent): create the key pair and add the secrets (`docs/release.md`).

### B. Legal
1. `PRIVACY.md` and `TERMS.md` at the repo root, written from what the code actually does (no telemetry, local storage, which actions reach the network, third-party providers).
2. Both ship inside the installer and app, show in Settings → About → Legal, and are linked from the setup wizard.
3. They are drafts for the owner to review. They are not legal advice.

### C. Fix the field artifacts
1. Draw focus rings on the field wrapper (`focus-within`) and stop the global outline on inputs inside wrappers.
2. Hide native search clear buttons, WebView2 reveal and clear buttons, and autofill tints.
3. A shared `Select` look: `appearance: none`, one chevron, themed options.
4. Replace Tailwind v3 `placeholder-ink-*` classes.

### D. Motion and coherence
1. Shared motion presets in `src/lib/motion.ts` (durations, easing), all honoring reduced motion.
2. Screen changes slide and fade; Tabs get a sliding indicator; settings sections and wizard steps transition; lists use `animate:flip`; cards lift on hover.
3. Command palette gains actions (new chat, theme, check for updates, open legal pages) and arrow-key navigation.
4. Mobile navigation gets a backdrop.

### E. Online providers
1. Key optional: providers that work without a key (LLM7, Pollinations, local routers) save without one; `RemoteEngine` sends no `Authorization` header when the key is empty.
2. Presets add 9Router and OmniRoute (`http://localhost:20128/v1`), Google Gemini (OpenAI-compatible endpoint), Hugging Face router, GitHub Models, Pollinations, and mark each preset as keyless or keyed.
3. `cloud_list_models`: `GET {base}/models` with the same URL rules, no redirects, a 2 MB cap and the same-origin key rule. The UI shows a searchable model list and marks free models where the provider says so.

### F. Hugging Face browsing
1. `hf_search` takes a sort (trending, downloads, likes, newest) and an optional tag filter (`uncensored`, `abliterated`).
2. The Models screen opens with trending GGUF models when the query is empty.
3. Unfiltered models carry a plain warning that their safety tuning was removed.

### G. Skills from GitHub
1. Sources: Anthropic Skills (`anthropics/skills`), GitHub Awesome Copilot (`github/awesome-copilot`), prompts.chat (`f/prompts.chat`), and any public `owner/repo` with a `skills/<name>/SKILL.md` layout.
2. Browsing is an explicit action; listings use one GitHub API call per source; skill files come from `raw.githubusercontent.com`. Each import records its source URL and license line.

### H. Verify
- `npm run check`, `npm run build`, `npm test` locally.
- `cargo check` and `cargo clippy` locally against stub system libraries (the sandbox has no WebKitGTK); `cargo test` and the platform bundles run in CI with a `[build-all]` push.

## Out of scope for this round
- Code signing certificates (needs paid Apple and Windows certificates).
- Arabic or other translations (ask first).
