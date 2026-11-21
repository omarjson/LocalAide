// The shared LocalAide page. Everything shown is built with textContent, so
// nothing from a conversation can run as code. The token comes from the
// link's #k= part (never sent to any server in the URL), is kept for this
// tab only, and goes with each request as a bearer token.
(() => {
  "use strict";
  const root = document.getElementById("root");
  const hash = new URLSearchParams(location.hash.slice(1));
  if (hash.get("k")) {
    sessionStorage.setItem("localaide-share-token", hash.get("k"));
    history.replaceState(null, "", location.pathname);
  }
  const token = sessionStorage.getItem("localaide-share-token") || "";

  const h = (tag, attrs = {}, ...kids) => {
    const el = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
      if (v === undefined || v === null || v === false) continue;
      if (k === "class") el.className = v;
      else if (k === "text") el.textContent = v;
      else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
      else el.setAttribute(k, v === true ? "" : v);
    }
    for (const kid of kids.flat()) if (kid !== null && kid !== undefined && kid !== false) el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
    return el;
  };

  // Small line icons, drawn as SVG so no font is needed.
  const icon = (d) => {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("class", "ico");
    svg.setAttribute("aria-hidden", "true");
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", d);
    svg.append(path);
    return svg;
  };
  const ICON_UP = "M12 19V5M6 11l6-6 6 6";
  const ICON_MENU = "M4 7h16M4 12h16M4 17h16";

  const api = async (path, opts = {}) => {
    const res = await fetch(path, { ...opts, headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json", ...(opts.headers || {}) } });
    if (!res.ok) {
      let msg = `The assistant answered ${res.status}.`;
      try { msg = (await res.json()).error || msg; } catch { /* not JSON */ }
      throw new Error(msg);
    }
    return res;
  };
  // Lines of JSON as they arrive.
  const stream = async (path, body, onEvent) => {
    const res = await api(path, { method: "POST", body: JSON.stringify(body) });
    const reader = res.body.getReader();
    const dec = new TextDecoder();
    let buf = "";
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true });
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl).trim();
        buf = buf.slice(nl + 1);
        if (line) { try { onEvent(JSON.parse(line)); } catch { /* skip a broken line */ } }
      }
    }
  };

  const center = (title, text) => {
    root.className = "center";
    root.replaceChildren(h("div", { class: "card" }, h("div", { class: "brand-mark centered" }, "L"), h("h2", { text: title }), h("p", { text: text })));
  };

  // Plain text into paragraphs, code blocks and inline code.
  const rich = (text) => {
    const out = [];
    const parts = String(text || "").split(/```[^\n]*\n?/);
    parts.forEach((chunk, i) => {
      if (i % 2 === 1) { out.push(h("pre", {}, h("code", { text: chunk.replace(/\n$/, "") }))); return; }
      for (const para of chunk.split(/\n{2,}/)) {
        if (!para.trim()) continue;
        const p = h("p");
        para.split(/(`[^`]+`|\*\*[^*]+\*\*)/).forEach((bit) => {
          if (/^`[^`]+`$/.test(bit)) p.append(h("code", { text: bit.slice(1, -1) }));
          else if (/^\*\*[^*]+\*\*$/.test(bit)) p.append(h("strong", { text: bit.slice(2, -2) }));
          else {
            const lines = bit.split("\n");
            lines.forEach((l, j) => { p.append(l); if (j < lines.length - 1) p.append(h("br")); });
          }
        });
        out.push(p);
      }
    });
    return out;
  };

  let me = null;
  let threads = [];
  let current = null; // { id, title, messages }
  let busy = false;
  let jobId = null;
  let keyHandler = null;

  const els = {};

  const optionButtons = (labels, onPick, tones = []) => {
    const wrap = h("div", { class: "opts" });
    const buttons = labels.map((label, i) => h("button", { class: `opt ${tones[i] === "primary" ? "primary" : ""}`, type: "button", onclick: () => pick(i) }, h("span", { class: "k", "aria-hidden": "true", text: String(i + 1) }), label));
    const pick = (i) => {
      buttons.forEach((b, j) => { b.disabled = true; if (j === i) b.classList.add("on"); });
      if (keyHandler) { document.removeEventListener("keydown", keyHandler); keyHandler = null; }
      onPick(labels[i]);
    };
    if (keyHandler) document.removeEventListener("keydown", keyHandler);
    keyHandler = (e) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.target === els.input && els.input.value !== "") return;
      const n = Number(e.key);
      if (Number.isInteger(n) && n >= 1 && n <= labels.length) { e.preventDefault(); pick(n - 1); }
    };
    document.addEventListener("keydown", keyHandler);
    wrap.append(...buttons);
    return wrap;
  };

  const toolCard = (part) => {
    const status = h("span", { class: "s" });
    const body = h("div", { class: "tool-body", hidden: true });
    const card = h("div", { class: "tool" }, h("div", { class: "tool-head" }, h("span", { class: "t", text: part.title || part.name }), h("span", { class: "d", text: part.detail || "" }), status), body);
    const set = (s, summary) => {
      status.className = `s ${s === "done" ? "ok" : s === "failed" || s === "denied" ? "bad" : s === "waiting" ? "wait" : ""}`;
      status.textContent = s === "running" ? "Working…" : s === "waiting" ? "Needs an OK" : s === "denied" ? "Not allowed" : summary || (s === "done" ? "Done" : "Failed");
      card.classList.toggle("waiting", s === "waiting");
    };
    set(part.status, part.summary);
    const showOutput = (output) => {
      if (!output) return;
      let text = output;
      if (part.group === "shell") { const nl = output.indexOf("\n"); if (nl >= 0 && output.startsWith("{")) text = output.slice(nl + 1); }
      body.hidden = false;
      body.replaceChildren(h("div", { class: "term", text: (part.group === "shell" ? `$ ${part.detail}\n` : "") + text }));
    };
    showOutput(part.output);
    return { card, set, showOutput, body };
  };

  const renderAnswer = (msg, live) => {
    const box = h("div", { class: "msg-ai", dir: "auto" });
    const parts = msg.parts && msg.parts.length ? msg.parts : [{ type: "text", text: msg.content }];
    for (const p of parts) {
      if (p.type === "text") box.append(...rich(p.text));
      else if (p.type === "tool") box.append(toolCard(p).card);
      else if (p.type === "question") box.append(h("p", {}, h("strong", { text: p.question })), h("p", { class: "meta", text: p.answer ? `Answered: ${p.answer}` : p.options.join(" · ") }));
    }
    if (msg.model) box.append(h("div", { class: "meta", text: `Answered by ${msg.model}` }));
    if (live) box.dataset.live = "1";
    return box;
  };

  const renderThread = () => {
    els.title.textContent = current ? current.title || "Chat" : "New chat";
    els.col.replaceChildren();
    const msgs = current ? current.messages : [];
    if (msgs.length === 0) {
      els.col.append(h("div", { class: "empty", text: me.role === "remote" ? "Ask anything. Commands and changes ask for your OK first." : "Pick a conversation on the left to read it." }));
    }
    for (const m of msgs) els.col.append(m.role === "user" ? h("div", { class: "msg-user", dir: "auto", text: m.content }) : renderAnswer(m));
    els.log.scrollTop = els.log.scrollHeight;
    for (const b of els.threads.children) b.setAttribute("aria-current", String(current && b.dataset.id === current.id));
  };

  const loadThreads = async () => {
    threads = await (await api("/api/threads")).json();
    els.threads.replaceChildren(...(threads.length ? threads.map((t) => h("button", { class: "thread", type: "button", "data-id": t.id, onclick: () => openThread(t), text: t.title || "Chat" })) : [h("p", { class: "meta", style: "padding:8px 10px", text: "No saved conversations yet." })]));
  };

  const openThread = async (t) => {
    els.side.classList.remove("open");
    const messages = await (await api(`/api/threads/${encodeURIComponent(t.id)}`)).json();
    current = { id: t.id, title: t.title, messages };
    renderThread();
  };

  const newChat = () => {
    current = { id: `t_${Date.now()}`, title: "New chat", messages: [] };
    renderThread();
    els.input?.focus();
  };

  const save = async () => {
    if (!current || me.role !== "remote") return;
    try {
      await api(`/api/threads/${encodeURIComponent(current.id)}`, { method: "POST", body: JSON.stringify({ title: current.title, messages: current.messages }) });
      // Refresh the list only; the answer on screen keeps its live buttons.
      await loadThreads();
      for (const b of els.threads.children) b.setAttribute("aria-current", String(b.dataset.id === current.id));
    } catch { /* the answer still shows */ }
  };

  const send = async (text) => {
    text = (text ?? els.input.value).trim();
    if (!text || busy) return;
    if (!current) newChat();
    els.input.value = "";
    const history = current.messages.filter((m) => m.content && !m.error).map((m) => ({ role: m.role, content: m.content }));
    current.messages.push({ role: "user", content: text, ts: Date.now() });
    if (current.messages.length === 1) current.title = text.slice(0, 60);
    const answer = { role: "assistant", content: "", ts: Date.now(), parts: [] };
    current.messages.push(answer);
    renderThread();
    const box = els.col.lastElementChild;
    box.replaceChildren(h("div", { class: "meta", text: "Thinking…" }));
    busy = true; els.send.disabled = true;
    const cards = {};
    let textEl = null;
    const textPart = () => {
      let last = answer.parts[answer.parts.length - 1];
      if (!last || last.type !== "text") { last = { type: "text", text: "" }; answer.parts.push(last); textEl = h("div"); box.append(textEl); }
      return last;
    };
    try {
      await stream("/api/chat", { message: text, model: "", history, now: new Date().toString() }, (ev) => {
        const p = ev.payload || {};
        if (box.firstChild && box.firstChild.className === "meta" && ev.type !== "status" && ev.type !== "job") box.firstChild.remove();
        if (ev.type === "job") jobId = p.job_id;
        else if (ev.type === "status" && p.label.startsWith("Answering with ")) answer.model = p.label.slice(15);
        else if (ev.type === "token") { const t = textPart(); t.text += p.token; answer.content += p.token; textEl.replaceChildren(...rich(t.text)); }
        else if (ev.type === "tool_call") { const part = { type: "tool", ...p, status: "running", summary: "", output: "" }; answer.parts.push(part); textEl = null; cards[p.id] = { part, ...toolCard(part) }; box.append(cards[p.id].card); }
        else if (ev.type === "tool_approval") {
          const c = cards[p.id]; if (!c) return;
          c.part.status = "waiting"; c.set("waiting");
          const shell = c.part.group === "shell";
          const labels = shell ? ["Run", "Allow for this chat", "Don't run"] : ["Allow once", "Allow for this chat", "Don't allow"];
          const decisions = ["allow", "allow_chat", "deny"];
          c.body.hidden = false;
          c.body.replaceChildren(h("div", { text: shell ? "Run this command on the owner's computer?" : "The assistant wants to do this. Nothing happens until you choose." }), h("div", { class: "term", text: p.preview }), optionButtons(labels, async (label) => {
            const decision = decisions[labels.indexOf(label)];
            if (decision === "deny") { c.part.status = "denied"; c.set("denied"); } else { c.part.status = "running"; c.set("running"); }
            await api("/api/decide", { method: "POST", body: JSON.stringify({ approval_id: p.approval_id, decision }) }).catch(() => {});
          }, ["primary"]));
        }
        else if (ev.type === "tool_result") { const c = cards[p.id]; if (!c) return; if (c.part.status !== "denied") c.part.status = p.ok ? "done" : "failed"; c.part.summary = p.summary; c.part.output = p.output; c.set(c.part.status, p.summary); c.showOutput(p.output); }
        else if (ev.type === "question") {
          answer.parts.push({ type: "question", ...p, answer: null });
          box.append(h("p", {}, h("strong", { text: p.question })), optionButtons(p.options, (label) => { answer.parts.find((x) => x.id === p.id).answer = label; setTimeout(() => send(label), 0); }));
        }
        else if (ev.type === "error") { answer.error = true; box.append(h("p", { class: "meta", style: "color:var(--bad)", text: p.error })); }
        els.log.scrollTop = els.log.scrollHeight;
      });
    } catch (e) {
      answer.error = true;
      box.append(h("p", { class: "meta", style: "color:var(--bad)", text: e.message }));
    } finally {
      busy = false; jobId = null; els.send.disabled = false;
      if (answer.model) box.append(h("div", { class: "meta", text: `Answered by ${answer.model}` }));
      await save();
    }
  };

  const selfTest = async () => {
    current = null;
    els.title.textContent = "Check my setup";
    const list = h("div", { class: "steps" });
    els.col.replaceChildren(h("p", { text: "Checks the owner's computer: the Tiny model is there (downloaded if missing), it loads, and it replies." }), list);
    const rows = {};
    const row = (s) => {
      if (!rows[s.step]) { rows[s.step] = h("div", { class: "step" }); list.append(rows[s.step]); }
      rows[s.step].className = `step ${s.status}`;
      const mark = s.status === "pass" ? "✓" : s.status === "fail" ? "✕" : s.status === "running" ? "…" : "–";
      rows[s.step].replaceChildren(h("span", { class: "n", text: mark }), h("div", {}, h("span", { text: `${s.label}${s.ms ? ` · ${(s.ms / 1000).toFixed(1)} s` : ""}` }), h("small", { text: s.percent !== null && s.status === "running" ? `${s.detail} ${Math.round(s.percent)}%` : s.detail })));
    };
    try {
      await stream("/api/selftest", {}, (ev) => {
        if (ev.type === "step") row(ev.payload);
        if (ev.type === "report") els.col.append(h("p", { style: `font-weight:600;color:${ev.payload.passed ? "var(--ok)" : "var(--bad)"}`, text: ev.payload.passed ? "Everything works." : "Something failed; see the step above." }));
      });
    } catch (e) {
      els.col.append(h("p", { style: "color:var(--bad)", text: e.message }));
    }
  };

  const build = () => {
    root.className = "app";
    const remote = me.role === "remote";
    els.threads = h("nav", { class: "threads", "aria-label": "Conversations" });
    els.side = h("aside", {},
      h("div", { class: "brand" }, h("div", { class: "brand-mark", "aria-hidden": "true" }, "L"), h("b", { text: "LocalAide" }), h("span", { class: "meta", text: `v${me.version}` })),
      h("div", { class: "role" }, "You are a ", h("strong", { text: remote ? "remote user" : "viewer" }), remote ? ". You can chat; commands and changes ask you first and are logged on the owner's computer." : ". You can read conversations."),
      els.threads,
      h("div", { class: "side-actions" }, remote ? h("button", { class: "btn btn-primary", type: "button", onclick: newChat, text: "New chat" }) : null, remote ? h("button", { class: "btn", type: "button", onclick: selfTest, text: "Check my setup" }) : null));
    els.title = h("h1", { text: "New chat" });
    const model = me.model || {};
    els.log = h("div", { class: "log" });
    els.col = h("div", { class: "col" });
    els.log.append(els.col);
    const main = h("main", {},
      h("header", {}, h("button", { class: "btn btn-quiet menu-btn", type: "button", "aria-label": "Conversations", onclick: () => els.side.classList.toggle("open") }, icon(ICON_MENU)), els.title, h("span", { class: "pill", text: model.online ? "Online model" : model.name ? `${model.name} · on the owner's computer` : "On the owner's computer" })),
      els.log);
    if (remote) {
      els.input = h("textarea", { rows: "1", placeholder: "Ask anything", "aria-label": "Message", dir: "auto", oninput: () => { els.input.style.height = "auto"; els.input.style.height = `${Math.min(els.input.scrollHeight, 200)}px`; }, onkeydown: (e) => { if (e.key === "Enter" && !e.shiftKey && !e.isComposing) { e.preventDefault(); send(); } if (e.key === "Escape" && busy && jobId) api("/api/cancel", { method: "POST", body: JSON.stringify({ job_id: jobId }) }).catch(() => {}); } });
      els.send = h("button", { class: "send", type: "button", "aria-label": "Send", onclick: () => send() }, icon(ICON_UP));
      main.append(h("div", { class: "composer" }, h("div", { class: "box" }, els.input, els.send), h("div", { class: "note", text: "Messages are answered on the owner's computer. Press Esc to stop an answer." })));
    }
    root.replaceChildren(els.side, main);
    renderThread();
  };

  // The pairing confirmation: the same XXXX-XXXX code the desktop shows.
  // Matching codes mean this phone and that computer are the same session.
  const showPairCode = async () => {
    try {
      const res = await (await api("/api/pair-code")).json();
      if (!res.code) return;
      const chip = document.createElement("div");
      chip.style.cssText = "position:fixed;left:10px;bottom:10px;z-index:9;background:var(--panel-2);border:1px solid var(--line-2);border-radius:10px;padding:6px 12px;font:600 14px/1 ui-monospace,monospace;letter-spacing:2px;color:var(--ink-2);opacity:0.9";
      chip.title = "This matches the code on your computer";
      chip.textContent = res.code;
      document.body.append(chip);
      setTimeout(() => { chip.style.transition = "opacity 1.2s"; chip.style.opacity = "0"; }, 6000);
    } catch (e) {
      // The code is a nicety; pairing still works without it.
    }
  };

  (async () => {
    if (!token) { center("This link needs its token", "Open the full link the owner gave you. It ends with #k= and a long code."); return; }
    try {
      me = await (await api("/api/me")).json();
      current = null;
      build();
      await loadThreads();
      void showPairCode();
    } catch (e) {
      center("The assistant did not let this link in", e.message);
    }
  })();
})();
