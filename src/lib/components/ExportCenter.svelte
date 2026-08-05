<script lang="ts">
  import {
    Archive,
    Bot,
    Boxes,
    Check,
    CircleAlert,
    Clock3,
    Download,
    FileJson,
    History,
    Layers3,
    RefreshCw,
    ShieldCheck,
    Sparkles,
    Trash2,
    X
  } from "@lucide/svelte";
  import type {
    AgentSkillResolution,
    AssetGroup,
    ExportFormat,
    ExportHistoryEntry,
    ExportMode
  } from "../types";
  import { formatLocaleDateTime, formatLocaleNumber, locale, tr } from "../i18n";

  export let exportMode: ExportMode;
  export let exportFormat: ExportFormat;
  export let exporting: boolean;
  export let analyzingRelations: boolean;
  export let relationPreview: AgentSkillResolution | null;
  export let selectedAgents: AssetGroup[];
  export let selectedSkills: AssetGroup[];
  export let plannedAssetCount: number;
  export let exportReady: boolean;
  export let chooseMode: (mode: ExportMode) => void;
  export let analyzeCompanions: () => void;
  export let goToAssets: () => void;
  export let runExport: () => void;
  export let exportHistory: ExportHistoryEntry[];
  export let clearExportHistory: () => void;

  function fileName(path: string): string {
    return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
  }

  function formatCompletedAt(value: string): string {
    return formatLocaleDateTime(value, $locale);
  }
</script>

<section class="compact-heading">
  <div>
    <span class="eyebrow">PORTABLE & AUDITABLE</span>
    <h1>{tr("export.title", {}, $locale)}</h1>
    <p>{tr("export.subtitle", {}, $locale)}</p>
  </div>
</section>

<section class="export-layout">
  <article class="panel export-builder">
    <div class="panel-heading">
      <div><span class="section-kicker">BUILD EXPORT</span><h2>{tr("export.build", {}, $locale)}</h2></div>
      <FileJson size={21} />
    </div>
    <div class="export-count">
      <strong>{formatLocaleNumber(plannedAssetCount, $locale)}</strong>
      <span>{tr("common.items", { count: plannedAssetCount }, $locale)} · {selectedAgents.length} Agent / {selectedSkills.length} Skill</span>
    </div>
    <div class="mode-options" aria-label={tr("export.title", {}, $locale)}>
      <button
        class:active={exportMode === "selection"}
        aria-pressed={exportMode === "selection"}
        onclick={() => chooseMode("selection")}
      >
        <Layers3 size={18} /><span><strong>{tr("export.selection", {}, $locale)}</strong><small>Agent / Skill</small></span>
      </button>
      <button
        class:active={exportMode === "agents_only"}
        aria-pressed={exportMode === "agents_only"}
        onclick={() => chooseMode("agents_only")}
      >
        <Bot size={18} /><span><strong>{tr("drawer.agentOnly", {}, $locale)}</strong><small>Agent</small></span>
      </button>
      <button
        class:active={exportMode === "skills_only"}
        aria-pressed={exportMode === "skills_only"}
        onclick={() => chooseMode("skills_only")}
      >
        <Sparkles size={18} /><span><strong>{tr("drawer.skillOnly", {}, $locale)}</strong><small>Skill</small></span>
      </button>
      <button
        class:active={exportMode === "agent_with_skills"}
        aria-pressed={exportMode === "agent_with_skills"}
        onclick={() => chooseMode("agent_with_skills")}
      >
        <Boxes size={18} /><span><strong>{tr("export.agentSkills", {}, $locale)}</strong><small>1 Agent + Skills</small></span>
      </button>
    </div>

    {#if exportMode === "agent_with_skills"}
      <div class="loadout-preview">
        <div class="loadout-heading">
          <div>
            <strong>{tr("export.agentSkills", {}, $locale)}</strong>
            <small>{tr("export.subtitle", {}, $locale)}</small>
          </div>
          <button
            class="secondary-button small"
            disabled={analyzingRelations || selectedAgents.length !== 1}
            onclick={analyzeCompanions}
          >
            <RefreshCw size={14} /> {tr(analyzingRelations ? "common.loading" : "common.refresh", {}, $locale)}
          </button>
        </div>
        {#if selectedAgents.length !== 1}
          <div class="loadout-warning"><CircleAlert size={15} />Agent: 1</div>
        {:else}
          <div class="loadout-agent">
            <span class="asset-glyph agent"><Bot size={15} /></span>
            <span><strong>{selectedAgents[0].name}</strong><small>Agent</small></span>
          </div>
          {#if selectedSkills.length > 0}
            <div class="companion-list">
              {#each selectedSkills as skill}
                <span><Sparkles size={13} />{skill.name}<em>{tr("common.selected", {}, $locale)}</em></span>
              {/each}
            </div>
          {/if}
          {#if relationPreview?.agent_logical_id === selectedAgents[0].logical_id}
            {#if relationPreview.skills.length > 0}
              <div class="companion-list">
                {#each relationPreview.skills as skill}
                  <span><Sparkles size={13} />{skill.name}<em>Agent</em></span>
                {/each}
              </div>
            {:else if selectedSkills.length === 0}
              <div class="loadout-warning"><CircleAlert size={15} />{tr("common.notLinked", {}, $locale)}</div>
            {/if}
            {#if relationPreview.unresolved_references.length > 0}
              <small class="relation-note">{tr("common.items", { count: relationPreview.unresolved_references.length }, $locale)} · {tr("common.notLinked", {}, $locale)}</small>
            {/if}
          {/if}
        {/if}
      </div>
    {/if}

    <div class="format-options">
      <label class:active={exportFormat === "json"}>
        <input type="radio" bind:group={exportFormat} value="json" />
        <span><FileJson size={20} /><strong>JSON</strong><small>{tr("export.subtitle", {}, $locale)}</small></span>
      </label>
      <label class:active={exportFormat === "markdown"}>
        <input type="radio" bind:group={exportFormat} value="markdown" />
        <span><Layers3 size={20} /><strong>Markdown</strong><small>{tr("export.subtitle", {}, $locale)}</small></span>
      </label>
      <label class:active={exportFormat === "bundle"}>
        <input type="radio" bind:group={exportFormat} value="bundle" />
        <span><Archive size={20} /><strong>ZIP</strong><small>{tr("export.subtitle", {}, $locale)}</small></span>
      </label>
    </div>
    <div class="export-actions">
      <button class="secondary-button" onclick={goToAssets}>{tr("export.continue", {}, $locale)}</button>
      <button class="primary-button" disabled={!exportReady} onclick={runExport}>
        <Download size={17} /> {tr(exporting ? "export.exporting" : "export.generate", {}, $locale)}
      </button>
    </div>
  </article>

  <article class="panel export-policy">
    <div class="panel-heading">
      <div><span class="section-kicker">SAFETY POLICY</span><h2>{tr("export.boundary", {}, $locale)}</h2></div>
      <ShieldCheck size={21} />
    </div>
    <ul>
      <li><Check size={15} /><span><strong>SHA-256</strong>{tr("export.subtitle", {}, $locale)}</span></li>
      <li><Check size={15} /><span><strong>Skill</strong>SKILL.md · references · scripts · assets · tests</span></li>
      <li><X size={15} /><span><strong>.env · tokens · keys</strong>{tr("common.none", {}, $locale)}</span></li>
      <li><CircleAlert size={15} /><span><strong>{tr("common.status", {}, $locale)}</strong>{tr("status.latestPolicy", {}, $locale)}</span></li>
    </ul>
    <div class="policy-footnote">
      <Clock3 size={15} />
      <span>{tr("export.subtitle", {}, $locale)}</span>
    </div>
  </article>

  <article class="panel export-history">
    <div class="panel-heading">
      <div><span class="section-kicker">RECEIPTS</span><h2>{tr("export.receipts", {}, $locale)}</h2></div>
      {#if exportHistory.length > 0}
        <button class="icon-button" aria-label={tr("common.clear", {}, $locale)} title={tr("common.clear", {}, $locale)} onclick={clearExportHistory}><Trash2 size={15} /></button>
      {:else}
        <History size={20} />
      {/if}
    </div>
    {#if exportHistory.length > 0}
      <div class="receipt-list">
        {#each exportHistory as receipt (receipt.completed_at + receipt.destination)}
          <article>
            <div class="receipt-title">
              <strong title={receipt.destination}>{fileName(receipt.destination)}</strong>
              <span>{receipt.format.toUpperCase()}</span>
            </div>
            <small>{formatCompletedAt(receipt.completed_at)} · {receipt.agent_count} Agent / {receipt.skill_count} Skill · {tr("common.items", { count: receipt.file_count }, $locale)}</small>
            <code title={receipt.sha256}>SHA-256 {receipt.sha256.slice(0, 12)}…</code>
            {#if receipt.warning_count > 0}<em>{tr("common.items", { count: receipt.warning_count }, $locale)}</em>{/if}
          </article>
        {/each}
      </div>
    {:else}
      <div class="empty-receipts"><History size={22} /><span>{tr("common.none", {}, $locale)}</span></div>
    {/if}
  </article>
</section>
