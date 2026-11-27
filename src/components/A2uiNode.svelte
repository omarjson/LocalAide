<script lang="ts">
  // One A2UI Basic Catalog component, drawn with LocalAide's own look, and
  // its children. Inputs write straight into the surface's data model
  // (two-way binding); a Button sends its action back to the chat.
  import A2uiNode from "./A2uiNode.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import MarkdownDoc from "./MarkdownDoc.svelte";
  import Modal from "./Modal.svelte";
  import { openInBrowser } from "../lib/links";
  import { orderOptionsRecommended } from "../lib/questions";
  import {
    boundPath,
    buildAction,
    childEntries,
    localAction,
    mediaSource,
    resolve,
    runChecks,
    setPointer,
    toBoolean,
    toText,
    type A2uiComponent,
    type JsonValue,
    type Scope,
    type Surface,
  } from "../lib/a2ui";

  interface Props {
    surface: Surface;
    id: string;
    scope: Scope;
    /** Nesting depth, to stop a component that contains itself. */
    depth?: number;
    /** Sends a pressed button's action as the user's next message. */
    onAction?: (message: string) => void;
    /** Set by a Row or Column parent, so `weight` applies. */
    inFlex?: boolean;
  }

  let { surface, id, scope, depth = 0, onAction, inFlex = false }: Props = $props();

  const MAX_DEPTH = 40;
  const node = $derived<A2uiComponent | undefined>(depth > MAX_DEPTH ? undefined : surface.components[id]);
  const prop = (name: string): unknown => (node ? node[name] : undefined);
  const value = (name: string): unknown => resolve(prop(name), scope);
  const text = (name: string): string => toText(value(name));

  // Accessibility attributes, explicit ones winning over inferred labels.
  const a11y = $derived.by(() => {
    const raw = node?.["@accessibility"];
    if (!raw || typeof raw !== "object") return {} as Record<string, string | boolean | undefined>;
    const r = raw as Record<string, unknown>;
    const label = r.label === undefined ? undefined : toText(resolve(r.label, scope));
    const description = r.description === undefined ? undefined : toText(resolve(r.description, scope));
    return {
      "aria-label": label || undefined,
      "aria-description": description || undefined,
      "aria-live": typeof r.live === "string" && r.live !== "off" ? r.live : undefined,
      "aria-hidden": r.hidden === undefined ? undefined : toBoolean(resolve(r.hidden, scope)) || undefined,
    } as Record<string, string | boolean | undefined>;
  });

  const weight = $derived(inFlex && typeof prop("weight") === "number" ? `flex: ${prop("weight")} 1 0%; min-width: 0;` : "");

  // Writes an input's value to the data model at its bound path.
  const write = (name: string, next: JsonValue) => {
    const path = boundPath(prop(name), scope);
    if (path) setPointer(surface.data, path, next);
  };

  // Inputs show their check message once the user has touched them.
  let touched = $state(false);
  const check = $derived(node ? runChecks(node, scope) : { valid: true });

  const JUSTIFY: Record<string, string> = { start: "flex-start", center: "center", end: "flex-end", spaceBetween: "space-between", spaceAround: "space-around", spaceEvenly: "space-evenly", stretch: "stretch" };
  const ALIGN: Record<string, string> = { start: "flex-start", center: "center", end: "flex-end", stretch: "stretch" };
  const flexStyle = (direction: "row" | "column") =>
    `display:flex;flex-direction:${direction};justify-content:${JUSTIFY[toText(prop("justify"))] ?? "flex-start"};align-items:${ALIGN[toText(prop("align"))] ?? "stretch"};${weight}`;

  // Basic Catalog icon names mapped onto LocalAide's own icon set.
  const ICONS: Record<string, IconName> = {
    accountCircle: "users", person: "users", add: "plus", arrowBack: "chevron-right", arrowForward: "chevron-right", attachFile: "paperclip",
    calendarToday: "clock", event: "clock", call: "chat", phone: "chat", camera: "image", photo: "image", check: "check", close: "close",
    delete: "trash", download: "download", upload: "arrow-up", edit: "pencil", error: "alert", warning: "alert", favorite: "heart",
    favoriteOff: "heart", folder: "folder", help: "help", home: "computer", info: "info", locationOn: "pin", lock: "lock", lockOpen: "unlock",
    mail: "chat", menu: "menu", moreVert: "more", moreHoriz: "more", notifications: "bolt", notificationsOff: "bolt", pause: "stop",
    play: "arrow-up", stop: "stop", payment: "key", print: "file", refresh: "refresh", search: "search", send: "arrow-up", settings: "settings",
    share: "link", shoppingCart: "layers", star: "star-filled", starHalf: "star", starOff: "star", visibility: "eye", visibilityOff: "eye",
    volumeUp: "volume", volumeDown: "volume", volumeMute: "volume", volumeOff: "volume", fastForward: "chevron-right", rewind: "undo",
    skipNext: "chevron-right", skipPrevious: "undo",
  };
  const iconName = $derived.by(() => {
    const raw = prop("name");
    const resolved = typeof raw === "object" && raw && "svgPath" in raw ? null : toText(resolve(raw, scope));
    return resolved === null ? null : (ICONS[resolved] ?? ("sparkle" as IconName));
  });
  const svgPath = $derived.by(() => {
    const raw = prop("name");
    if (raw && typeof raw === "object" && "svgPath" in raw) return toText(resolve((raw as Record<string, unknown>).svgPath, scope));
    return null;
  });

  // The visible label of a button, for the readable line of its action.
  const labelOf = (childId: unknown): string => {
    const child = typeof childId === "string" ? surface.components[childId] : undefined;
    if (!child) return "";
    if (child.component === "Text") return toText(resolve(child.text, scope)).replace(/[#*_`]/g, "").trim();
    if (typeof child.child === "string") return labelOf(child.child);
    if (Array.isArray(child.children)) return child.children.map(labelOf).filter(Boolean).join(" ");
    return "";
  };

  const press = (e: MouseEvent) => {
    if (!node) return;
    const local = localAction(node, scope);
    if (local) {
      openInBrowser(e, local.openUrl);
      return;
    }
    const event = buildAction(surface, node, scope, labelOf(prop("child")));
    if (event && onAction) onAction(event.message);
  };

  // Tabs and Modal keep their own little state.
  let tab = $state(0);
  let modalOpen = $state(false);
  let filter = $state("");

  const options = $derived.by(() => {
    const raw = prop("options");
    if (!Array.isArray(raw)) return [] as { label: string; value: string; recommended: boolean }[];
    // A form may mark one option recommended; it stands first, marked.
    // Nothing is invented: an option without the flag stays unmarked.
    return orderOptionsRecommended(
      raw
        .filter((o): o is Record<string, unknown> => !!o && typeof o === "object")
        .map((o) => ({ label: toText(resolve(o.label, scope)), value: toText(o.value), recommended: o.recommended === true })),
    );
  });
  const selected = $derived.by(() => {
    const v = value("value");
    return Array.isArray(v) ? v.map(toText) : typeof v === "string" && v ? [v] : [];
  });
  const toggleOption = (option: string) => {
    const multiple = toText(prop("variant")) === "multipleSelection";
    const next = multiple ? (selected.includes(option) ? selected.filter((s) => s !== option) : [...selected, option]) : [option];
    write("value", next);
    touched = true;
  };

  // DateTimeInput: ISO 8601 in the data model, the native control's format on screen.
  const dateKind = $derived(prop("enableDate") === true && prop("enableTime") === true ? "datetime-local" : prop("enableTime") === true ? "time" : "date");
  const dateShown = $derived.by(() => {
    const raw = text("value");
    if (!raw) return "";
    if (dateKind === "time") return /^\d{2}:\d{2}/.exec(raw.includes("T") ? raw.split("T")[1] : raw)?.[0] ?? "";
    const d = new Date(raw);
    if (Number.isNaN(d.getTime())) return "";
    const pad = (n: number) => String(n).padStart(2, "0");
    const day = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
    return dateKind === "date" ? day : `${day}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
  });
  const writeDate = (raw: string) => {
    if (!raw) return write("value", "");
    if (dateKind === "time") return write("value", raw.length === 5 ? `${raw}:00` : raw);
    if (dateKind === "date") return write("value", raw);
    const d = new Date(raw);
    write("value", Number.isNaN(d.getTime()) ? raw : d.toISOString());
  };

  const labelText = $derived(text("label"));
  const inputId = $derived(`a2ui-${surface.id}-${id}-${scope.base.replace(/\W/g, "_")}`);
</script>

{#if node}
  {#if node.component === "Text"}
    <div class={`a2ui-leaf ${prop("variant") === "caption" ? "text-xs italic text-ink-400" : "text-ink-100"}`} style={weight} {...a11y}>
      <MarkdownDoc source={text("text")} />
    </div>
  {:else if node.component === "Image"}
    {@const media = mediaSource(value("url"))}
    {@const variant = toText(prop("variant")) || "mediumFeature"}
    {#if media.inline}
      <img src={media.inline} alt={text("description")} class={`a2ui-leaf a2ui-img-${variant}`} style={`object-fit:${{ contain: "contain", cover: "cover", fill: "fill", none: "none", scaleDown: "scale-down" }[toText(prop("fit"))] ?? "cover"};${weight}`} {...a11y} />
    {:else if media.remote}
      <button type="button" class="a2ui-leaf a2ui-link" style={weight} onclick={(e) => openInBrowser(e, media.remote ?? "")} title={media.remote}><Icon name="image" class="h-4 w-4" />{text("description") || "Open the image in your browser"}</button>
    {/if}
  {:else if node.component === "Video" || node.component === "AudioPlayer"}
    {@const media = mediaSource(value("url"))}
    {#if media.remote}
      <button type="button" class="a2ui-leaf a2ui-link" style={weight} onclick={(e) => openInBrowser(e, media.remote ?? "")} title={media.remote}><Icon name={node.component === "Video" ? "window" : "volume"} class="h-4 w-4" />{text("description") || (node.component === "Video" ? "Play the video in your browser" : "Play the audio in your browser")}</button>
    {:else if media.inline}
      <p class="a2ui-leaf text-xs text-ink-500">This {node.component === "Video" ? "video" : "audio"} is embedded and cannot play in the chat.</p>
    {/if}
  {:else if node.component === "Icon"}
    <span class="a2ui-leaf inline-flex text-ink-200" style={weight} {...a11y}>
      {#if svgPath}
        <svg viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor" aria-hidden="true"><path d={svgPath} /></svg>
      {:else}
        <Icon name={iconName ?? "sparkle"} class="h-5 w-5" />
      {/if}
    </span>
  {:else if node.component === "Row" || node.component === "Column"}
    <div style={flexStyle(node.component === "Row" ? "row" : "column")} class={node.component === "Row" ? "flex-wrap" : ""} {...a11y}>
      {#each childEntries(prop("children"), scope) as child (child.key)}
        <A2uiNode {surface} id={child.id} scope={child.scope} depth={depth + 1} {onAction} inFlex />
      {/each}
    </div>
  {:else if node.component === "List"}
    {@const horizontal = prop("direction") === "horizontal"}
    <div class={horizontal ? "a2ui-list-h" : "a2ui-list-v"} style={`align-items:${ALIGN[toText(prop("align"))] ?? "stretch"};${weight}`} role="list" {...a11y}>
      {#each childEntries(prop("children"), scope) as child (child.key)}
        <div role="listitem" class={horizontal ? "max-w-[18rem] shrink-0" : ""}><A2uiNode {surface} id={child.id} scope={child.scope} depth={depth + 1} {onAction} /></div>
      {/each}
    </div>
  {:else if node.component === "Card"}
    <div class="a2ui-leaf a2ui-card" style={weight} {...a11y}>
      {#if typeof prop("child") === "string"}<A2uiNode {surface} id={prop("child") as string} {scope} depth={depth + 1} {onAction} />{/if}
    </div>
  {:else if node.component === "Tabs"}
    {@const tabs = Array.isArray(prop("tabs")) ? (prop("tabs") as Record<string, unknown>[]) : []}
    <div class="a2ui-leaf" style={weight} {...a11y}>
      <div class="flex gap-1 border-b border-ink-800" role="tablist">
        {#each tabs as t, i (i)}
          <button type="button" role="tab" aria-selected={tab === i} class={`a2ui-tab ${tab === i ? "a2ui-tab-on" : ""}`} onclick={() => (tab = i)}>{toText(resolve(t.title, scope))}</button>
        {/each}
      </div>
      {#if tabs[tab] && typeof tabs[tab].child === "string"}
        <div class="pt-2" role="tabpanel"><A2uiNode {surface} id={tabs[tab].child as string} {scope} depth={depth + 1} {onAction} /></div>
      {/if}
    </div>
  {:else if node.component === "Modal"}
    <!-- The trigger opens the dialog; its own action does not run. -->
    <div style={weight} role="presentation" onclickcapture={(e) => { e.preventDefault(); e.stopPropagation(); modalOpen = true; }}>
      {#if typeof prop("trigger") === "string"}<A2uiNode {surface} id={prop("trigger") as string} {scope} depth={depth + 1} />{/if}
    </div>
    {#if modalOpen && typeof prop("content") === "string"}
      <Modal title={labelOf(prop("trigger")) || "Details"} onClose={() => (modalOpen = false)}>
        <A2uiNode {surface} id={prop("content") as string} {scope} depth={depth + 1} {onAction} />
      </Modal>
    {/if}
  {:else if node.component === "Divider"}
    {#if prop("axis") === "vertical"}
      <span class="mx-2 w-px self-stretch bg-ink-700" aria-hidden="true"></span>
    {:else}
      <hr class="my-2 border-ink-700" />
    {/if}
  {:else if node.component === "Button"}
    {@const variant = toText(prop("variant")) || "default"}
    {@const blocked = !check.valid || (!onAction && !localAction(node, scope))}
    <button
      type="button"
      class={`a2ui-leaf a2ui-btn a2ui-btn-${variant}`}
      style={weight}
      disabled={blocked}
      title={!check.valid ? check.message : !onAction ? "Open this chat to use the button" : undefined}
      onclick={press}
      {...a11y}
    >
      {#if typeof prop("child") === "string"}<A2uiNode {surface} id={prop("child") as string} {scope} depth={depth + 1} />{/if}
    </button>
  {:else if node.component === "TextField"}
    {@const variant = toText(prop("variant")) || "shortText"}
    <div class="a2ui-leaf flex flex-col gap-1" style={weight}>
      <label for={inputId} class="text-xs font-medium text-ink-300">{labelText}</label>
      {#if variant === "longText"}
        <textarea id={inputId} rows="3" class="a2ui-field" placeholder={text("placeholder")} value={text("value")} oninput={(e) => write("value", e.currentTarget.value)} onblur={() => (touched = true)} aria-invalid={touched && !check.valid} {...a11y}></textarea>
      {:else}
        <input
          id={inputId}
          class="a2ui-field"
          type={variant === "obscured" ? "password" : variant === "number" ? "number" : "text"}
          placeholder={text("placeholder")}
          value={text("value")}
          oninput={(e) => {
            const raw = e.currentTarget.value;
            write("value", variant === "number" && raw.trim() !== "" && !Number.isNaN(Number(raw)) ? Number(raw) : raw);
          }}
          onblur={() => (touched = true)}
          aria-invalid={touched && !check.valid}
          {...a11y}
        />
      {/if}
      {#if touched && !check.valid && check.message}<span class="text-xs text-rose-300">{check.message}</span>{/if}
    </div>
  {:else if node.component === "CheckBox"}
    <label class="a2ui-leaf flex cursor-pointer items-center gap-2 text-sm text-ink-100" style={weight}>
      <input type="checkbox" class="a2ui-check" checked={toBoolean(value("value"))} onchange={(e) => { write("value", e.currentTarget.checked); touched = true; }} {...a11y} />
      <span>{labelText}</span>
    </label>
    {#if touched && !check.valid && check.message}<span class="a2ui-leaf text-xs text-rose-300">{check.message}</span>{/if}
  {:else if node.component === "ChoicePicker"}
    {@const multiple = prop("variant") === "multipleSelection"}
    {@const chips = prop("displayStyle") === "chips"}
    {@const shown = prop("filterable") === true && filter.trim() ? options.filter((o) => o.label.toLowerCase().includes(filter.trim().toLowerCase())) : options}
    <fieldset class="a2ui-leaf" style={weight} {...a11y}>
      {#if labelText}<legend class="mb-1 text-xs font-medium text-ink-300">{labelText}</legend>{/if}
      {#if prop("filterable") === true}<input class="a2ui-field mb-1.5" type="search" placeholder="Filter…" aria-label="Filter the options" bind:value={filter} />{/if}
      <div class={chips ? "flex flex-wrap gap-1.5" : "flex flex-col gap-1"} role={multiple ? "group" : "radiogroup"}>
        {#each shown as option (option.value)}
          {#if chips}
            <button type="button" role={multiple ? "checkbox" : "radio"} aria-checked={selected.includes(option.value)} class={`a2ui-chip ${selected.includes(option.value) ? "a2ui-chip-on" : ""}`} onclick={() => toggleOption(option.value)}>{option.label}{#if option.recommended}<span class="a2ui-rec">Recommended</span>{/if}</button>
          {:else}
            <label class="flex cursor-pointer items-center gap-2 text-sm text-ink-100">
              <input type={multiple ? "checkbox" : "radio"} class="a2ui-check" name={inputId} checked={selected.includes(option.value)} onchange={() => toggleOption(option.value)} />
              <span>{option.label}{#if option.recommended}<span class="a2ui-rec">Recommended</span>{/if}</span>
            </label>
          {/if}
        {/each}
      </div>
      {#if touched && !check.valid && check.message}<span class="text-xs text-rose-300">{check.message}</span>{/if}
    </fieldset>
  {:else if node.component === "Slider"}
    {@const min = Number(value("min") ?? 0) || 0}
    {@const max = Number(value("max")) || 100}
    {@const steps = Number(prop("steps")) || 0}
    {@const current = Number(value("value") ?? min)}
    <div class="a2ui-leaf flex flex-col gap-1" style={weight}>
      <div class="flex items-center justify-between text-xs text-ink-300"><label for={inputId}>{labelText}</label><span class="tabular-nums text-ink-200">{Number.isFinite(current) ? current : min}</span></div>
      <input id={inputId} type="range" {min} {max} step={steps > 0 ? (max - min) / steps : "any"} value={Number.isFinite(current) ? current : min} oninput={(e) => write("value", Number(e.currentTarget.value))} {...a11y} />
    </div>
  {:else if node.component === "DateTimeInput"}
    <div class="a2ui-leaf flex flex-col gap-1" style={weight}>
      {#if labelText}<label for={inputId} class="text-xs font-medium text-ink-300">{labelText}</label>{/if}
      <input id={inputId} class="a2ui-field" type={dateKind} value={dateShown} onchange={(e) => { writeDate(e.currentTarget.value); touched = true; }} {...a11y} />
    </div>
  {/if}
{/if}

<style>
  .a2ui-leaf { margin: 0.25rem; }
  .a2ui-card { border: 1px solid var(--color-edge); border-radius: 0.75rem; padding: 0.75rem; }
  .a2ui-list-v { display: flex; flex-direction: column; max-height: 22rem; overflow-y: auto; }
  .a2ui-list-h { display: flex; flex-direction: row; overflow-x: auto; scrollbar-width: none; }
  .a2ui-btn { display: inline-flex; align-items: center; justify-content: center; gap: 0.35rem; border-radius: 0.6rem; padding: 0.25rem 0.85rem; font-size: 0.875rem; font-weight: 500; transition: background-color 120ms ease-out, opacity 120ms ease-out; }
  .a2ui-btn :global(.a2ui-leaf) { margin: 0; }
  .a2ui-btn :global(p) { margin: 0; }
  .a2ui-btn-default { border: 1px solid var(--color-edge); background: color-mix(in oklab, var(--color-veil) 5%, transparent); color: var(--color-ink-50); }
  .a2ui-btn-default:hover:not(:disabled) { background: color-mix(in oklab, var(--color-veil) 10%, transparent); }
  .a2ui-btn-primary { background: var(--color-ink-100); color: var(--color-ink-950); }
  .a2ui-btn-primary:hover:not(:disabled) { background: var(--color-ink-50); }
  .a2ui-btn-borderless { color: var(--color-ink-300); padding-inline: 0.35rem; }
  .a2ui-btn-borderless:hover:not(:disabled) { text-decoration: underline; }
  .a2ui-btn:disabled { cursor: not-allowed; opacity: 0.45; }
  .a2ui-field { width: 100%; border: 1px solid var(--color-edge); border-radius: 0.5rem; background: var(--color-ink-950); padding: 0.4rem 0.6rem; font-size: 0.875rem; color: var(--color-ink-50); }
  .a2ui-field:focus { outline: none; border-color: color-mix(in oklab, var(--color-ember-500) 60%, transparent); }
  .a2ui-field[aria-invalid="true"] { border-color: rgb(253 164 175 / 0.6); }
  .a2ui-check { accent-color: var(--color-ember-500); height: 1rem; width: 1rem; }
  .a2ui-chip { border: 1px solid var(--color-edge); border-radius: 9999px; padding: 0.2rem 0.7rem; font-size: 0.8125rem; color: var(--color-ink-200); }
  .a2ui-chip-on { border-color: var(--color-ink-600); background: var(--color-ink-700); color: var(--color-ink-50); }
  .a2ui-rec { margin-left: 0.35rem; border-radius: 9999px; background: var(--color-ink-700); padding: 0.05rem 0.45rem; font-size: 0.6875rem; font-weight: 600; color: var(--color-ink-200); white-space: nowrap; }
  .a2ui-tab { padding: 0.35rem 0.75rem; font-size: 0.8125rem; color: var(--color-ink-400); border-bottom: 2px solid transparent; margin-bottom: -1px; }
  .a2ui-tab-on { color: var(--color-ink-50); border-bottom-color: var(--color-ember-500); font-weight: 600; }
  .a2ui-link { display: inline-flex; align-items: center; gap: 0.4rem; font-size: 0.8125rem; color: var(--color-ember-300); }
  .a2ui-link:hover { text-decoration: underline; }
  .a2ui-img-icon { height: 1.5rem; width: 1.5rem; }
  .a2ui-img-avatar { height: 2.5rem; width: 2.5rem; border-radius: 9999px; }
  .a2ui-img-smallFeature { height: 6.25rem; width: 6.25rem; border-radius: 0.5rem; }
  .a2ui-img-mediumFeature { width: 100%; max-width: 18.75rem; border-radius: 0.5rem; }
  .a2ui-img-largeFeature { width: 100%; max-height: 25rem; border-radius: 0.5rem; }
  .a2ui-img-header { width: 100%; height: 12.5rem; border-radius: 0.5rem; }
</style>
