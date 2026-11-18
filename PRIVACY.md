# LocalAide Privacy Policy

Effective date: November 18, 2026. Applies to LocalAide 0.2.0 and later.

LocalAide is a desktop app that runs AI models on your own computer. This policy explains what the app does with your information. It is written from what the app's code actually does.

## The short version

- LocalAide has no account, no analytics, no crash reporting and no advertising.
- Your chats, notes, documents and settings are stored on your computer and nowhere else.
- The app connects to the internet only when you ask it to, for the actions listed below.
- The developer of LocalAide does not receive, see or store anything you do in the app.

## What is stored on your computer

Everything lives in a folder named `.aiassistant` in your home folder:

| What | Where |
|---|---|
| Chats | `~/.aiassistant/conversations/` |
| Memory notes | `~/.aiassistant/memory/` |
| Your document library (text taken from files you add, and its search index) | `~/.aiassistant/index/library.db` |
| Saved skills (prompts) | `~/.aiassistant/skills/` |
| Downloaded models | `~/.aiassistant/models/` |
| Settings, including any API keys you enter | `~/.aiassistant/config.json` |
| A log of what the app did, and a log of file actions the assistant took | `~/.aiassistant/logs/` and the library database |

These files stay on your computer when you update or uninstall the app. You can delete them at any time by deleting the folder. Settings → Privacy & files also has controls to clear memory and the activity log.

API keys are stored as plain text in `config.json`, protected by your computer's user account. Backups leave keys out unless you choose to include them, and backups are encrypted with a passphrase you choose.

## When LocalAide uses the internet

Nothing is contacted when the app starts. The app connects only when you take one of these actions:

| Your action | Who is contacted | What is sent |
|---|---|---|
| Download a model, refresh the model catalog, or browse Hugging Face | Hugging Face (`huggingface.co`) and GitHub (`raw.githubusercontent.com`) | The model or search words you chose |
| Search the web or read a web page from the Search screen | The search provider you picked (for example DuckDuckGo, Wikipedia, Jina, Exa or Firecrawl) | Your search words or the page address, plus your key for that provider if it needs one |
| Browse or import skills | GitHub (`api.github.com`, `raw.githubusercontent.com`) | The name of the collection you opened |
| Check for updates, or install an update | GitHub (`github.com`, `api.github.com`, `objects.githubusercontent.com`) | A request for the newest release; nothing about you or your data |
| Turn on an online provider and chat | The provider you added (for example OpenRouter, Groq, Google Gemini, or a router on your own computer) | Your messages, the chat history the model needs, and your key for that provider |

Like any website, each service sees your computer's IP address when the app connects. Each of these services has its own privacy policy, and the developer of LocalAide has no control over them. Read their policies before you send them anything sensitive. Requests go straight from your computer to that service. They do not pass through any server run by the developer of LocalAide.

When online chat is off (the default), your messages are processed only by the model on your computer.

## Children

LocalAide is not directed at children under 13 and collects no information from anyone.

## Changes to this policy

A new version of this policy ships with the app version it applies to and is listed in `CHANGELOG.md`. The effective date at the top changes when the policy does.

## Contact

Questions about this policy: open an issue at https://github.com/omarjson/LocalAide/issues. To report a security problem privately, use https://github.com/omarjson/LocalAide/security/advisories/new.
