# LocalAide 0.1.7: tasks

- [x] 1. Engine downloaded per processor; remove the bundled sidecar; ADR-0012, docs, privacy table. _Verified: `real_llama_fetch` downloaded b12359 for Linux x64 here and `llama-server --version` printed build 12359; pins checked against GitHub's digests._
- [x] 2. Windows ARM64 in CI, update feed and release notes; sherpa-onnx ARM64 pin; cloudflared uses x64 under emulation.
- [x] 3. Chat kept mounted; scroll restore; Settings section remembered. _Verified in the browser preview: Chat → Settings → Chat keeps the conversation; Settings reopens on Web search._
- [x] 4. Sidebar chats by folder; folder jail; refusing root, home and the app data folder.
- [x] 5. Permission modes with Plan → Build; "How LocalAide answers" out of the chat box. _Unit test `each_mode_asks_as_its_menu_says`, `autonomous_mode_asks_only_about_risky_commands`._
- [x] 6. Model menu with provider view and effort slider; Ctrl+T.
- [x] 7. Question card with several questions, descriptions, Other, Dismiss, deferred note. _`questions_come_in_both_forms`._
- [x] 8. Subagents with steps, work status and panel.
- [x] 9. Charts, Mermaid diagrams, sandboxed mockups with picks. _`pages_are_served_with_a_strict_policy`._
- [x] 10. Learned skills with Undo. _`learned_skills_can_be_undone`._
- [x] 11. Suggested next prompt, improve prompt.
- [x] 12. Learned starter prompts. _`starters_come_from_how_chats_begin`._
- [x] 13. File actions and folder in ⋯ menu. _`scripts_are_not_opened_by_default`._
- [x] 14. Interface scale 70–250%.
- [x] 15. DMG background, 2x at 144 dpi.
- [x] 16. Motion and polish pass (ui-skills).
- [x] 17. CHANGELOG, screenshots in `docs/evidence/0.1.7/`, draft PR, CI, draft release.

Not checked here (no GUI, no real model download from Hugging Face in this sandbox): a real model answering through the downloaded engine, the ARM64 installer on a device, mockup frames inside the real webview, Finder / File Explorer actions, and the DMG window on a Mac.
