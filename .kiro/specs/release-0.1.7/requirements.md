# LocalAide 0.1.7: requirements

From the owner's report after 0.1.6 (a Windows PC where every model failed with `exit code: 0xc000001d`) and the feature list that came with it. Reference: the owner's screenshots, OpenChamber (MIT) for questions and rendered pages, ui-skills.com (baseline-ui, fixing-motion-performance, animate) for the look and motion.

1. **Engine for every processor.** WHEN a model loads and llama-server is missing THEN the app SHALL download the official llama.cpp build for this processor (native architecture, also under emulation), check its SHA-256, and use it. Installers SHALL NOT ship llama-server. A processor without AVX2 SHALL be able to run models.
2. **Windows on ARM.** CI SHALL build an ARM64 Windows installer and update package.
3. **Navigation keeps state.** Leaving Chat and coming back SHALL keep the open chat, its scroll position and a running answer. Settings SHALL reopen on the section the user left.
4. **Chats in the sidebar**, grouped by project folder, plus plain chats. A chat started in a folder SHALL use that folder for file tools and commands. The disk root, the home folder and LocalAide's data folder SHALL be refused.
5. **Permission icon in the chat box** with Plan mode, Ask before changes, Autonomous (asks only on risk) and Full access. Plan mode SHALL only read and SHALL offer Build afterwards. "How LocalAide answers" SHALL leave the chat box (it stays in Chat settings).
6. **Model menu** as in the screenshots: provider row (switch, New provider, Manage models), models with search, effort slider; Ctrl+T steps the effort.
7. **Questions** as a card: several questions, options with descriptions, Other, Dismiss, Submit, and a note when the user sent a message before answering.
8. **Subagents**: every subagent row SHALL show its current step on a second line; a Subagents panel SHALL list them all with steps and report. Subagents SHALL NOT change anything.
9. **Charts and mockups** in answers, sandboxed from the user's files and the network; a mockup MAY send the user's pick as the next message.
10. **Learned skills**: the model SHALL save and improve skills on its own, with a note in the chat and Undo.
11. **Suggested next prompt** after an answer, and an **improve prompt** icon next to Send.
12. **Starter prompts** learned from the user's chats, on this computer.
13. **Files**: open, open with, and show in Finder / File Explorer for files a chat wrote; the ⋯ menu opens the chat's folder. Programs and scripts SHALL NOT open with their default app.
14. **Interface scale** 70% to 250%.
15. **DMG**: a nicer, Retina-sharp background.
16. **Look and motion** per the installed ui-skills: hover gated to real pointers, short transitions for frequent actions, no `transition: all`, minimum text size kept.
