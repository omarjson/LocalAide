            <h3 class=\"mb-2 mt-7 flex items-center gap-2 text-[15px] font-semibold text-ink-50\"><Icon name=\"flask\" class=\"h-4 w-4 text-ember-300\" />Experimental features</h3>
            <div class={`space-y-2 transition-opacity ${ds.enabled ? "" : "pointer-events-none opacity-50"}`} aria-disabled={!ds.enabled}>
              {#each ds.flag_list as f (f.id)}
                <div class=\"flex items-center gap-2 rounded-md border border-ink-700 bg-ink-900 p-3.5 transition-colors hover:border-edge\">
                  <span class=\"min-w-0\"><span class=\"block text-sm font-semibold text-ink-100\">{f.label}</span><span class=\"block text-sm text-ink-400\">{f.description}</span></span>
                  <Switch checked={!!ds.flags[f.id]} label={f.label} disabled={!ds.enabled || devBusy} onchange={(on) => void setDev({ flags: { [f.id]: on } })} />
                </div>
              {/each}
            </div>

            {#if activeEffects.length > 0}
              <h3 class=\"mb-2 mt-7 flex items-center gap-2 text-[15px] font-semibold text-ink-50\"><Icon name=\"panel\" class=\"h-4 w-4 text-ember-300\" />Active effects</h3>
              <p class=\"mb-2 text-sm text-ink-400\">These flags are changing the interface right now. Each is also a <code class=\"font-mono text-ink-200\">data-…</code> attribute on the page.</p>
              <div class=\"flex flex-wrap gap-2\">
                {#each activeEffects as id (id)}
                  <span class=\"flex items-center gap-1.5 rounded-md border border-ember-500/30 bg-ember-500/10 px-2 py-1 font-mono text-xs text-ember-200\">
                    <Icon name={EFFECT_ICONS[id] ?? "flask"} class=\"h-3.5 w-3.5\" />{id}
                  </span>
                {/each}
              </div>
            {/if}

            <h3 class=\"mb-2 mt-7 flex items-center gap-2 text-[15px] font-semibold text-ink-50\"><Icon name=\"layers\" class=\"h-4 w-4 text-ember-300\" />Modules</h3>
            <p class=\"mb-2 text-sm text-ink-400\">Switch off parts of LocalAide you do not use. They stay off until you switch them on here, even outside developer mode.</p>