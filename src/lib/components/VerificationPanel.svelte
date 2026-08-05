<script lang="ts">
  import { onMount } from "svelte";
  import { CheckCircle2, ChevronRight, FlaskConical, RefreshCw, ShieldAlert } from "@lucide/svelte";
  import { runAssetAudit } from "../api";
  import type { AssetAuditReport, AssetAuditResult, AssetGroup } from "../types";
  import { formatLocaleNumber, locale, tr } from "../i18n";

  let { groups, openAsset, reportError }: {
    groups: AssetGroup[];
    openAsset: (asset: AssetGroup) => void;
    reportError: (reason: unknown) => void;
  } = $props();
  let result = $state<AssetAuditResult | null>(null);
  let running = $state(false);
  let query = $state("");
  let expanded = $state("");
  let filtered = $derived((result?.reports ?? []).filter((report) => `${report.name} ${report.kind}`.toLowerCase().includes(query.toLowerCase())));

  async function run(deep: boolean) {
    running = true;
    try { result = await runAssetAudit([], deep); }
    catch (reason) { reportError(reason); }
    finally { running = false; }
  }

  function showAsset(report: AssetAuditReport) {
    const asset = groups.find((item) => item.logical_id === report.logical_id);
    if (asset) openAsset(asset);
  }

  onMount(() => run(false));
</script>

<section class="audit-summary-grid">
  <article class="audit-intro"><span class="section-kicker">READ-ONLY AUTO AUDIT</span><h2>{tr("audit.title", {}, $locale)}</h2><p>{tr("controls.safety", {}, $locale)}</p><div><button class="secondary-button small" disabled={running} onclick={() => run(false)}><RefreshCw size={15} />{tr("audit.light", {}, $locale)}</button><button class="primary-button small" disabled={running} onclick={() => run(true)}><FlaskConical size={15} />{tr("audit.deep", {}, $locale)}</button></div></article>
  <article><span>{tr("common.assets", {}, $locale)}</span><strong>{result?.total ?? groups.length}</strong><small>{result ? `${result.duration_ms}ms` : tr("common.loading", {}, $locale)}</small></article>
  <article class="good"><span>{tr("audit.usable", {}, $locale)}</span><strong>{result?.usable ?? "—"}</strong></article>
  <article class="warn"><span>{tr("audit.optimize", {}, $locale)}</span><strong>{result?.needs_attention ?? "—"}</strong></article>
  <article class="bad"><span>{tr("audit.blocked", {}, $locale)}</span><strong>{result?.blocked ?? "—"}</strong></article>
</section>

<section class="panel audit-panel">
  <header class="control-panel-heading audit-heading"><div><span class="section-kicker">LOWEST SCORE FIRST</span><h2>{tr("projects.attention", {}, $locale)}</h2></div><input bind:value={query} placeholder={tr("common.search", {}, $locale)} /></header>
  {#if running}<div class="control-loading">{tr("common.loading", {}, $locale)} {formatLocaleNumber(groups.length, $locale)}</div>
  {:else if result}
    <div class="audit-list">
      {#each filtered.slice(0, 80) as report}
        <article class:open={expanded === report.logical_id}>
          <button class="audit-row" onclick={() => (expanded = expanded === report.logical_id ? "" : report.logical_id)}>
            <span class="audit-score {report.status}">{report.score}</span>
            <span class="audit-copy"><strong>{report.name}</strong><small>{report.kind} · {report.suggestions[0] || tr("audit.usable", {}, $locale)}</small></span>
            <span class="binding-state {report.status}">{tr(report.status === "usable" ? "audit.usable" : report.status === "blocked" ? "audit.blocked" : "audit.optimize", {}, $locale)}</span><ChevronRight size={15} />
          </button>
          {#if expanded === report.logical_id}
            <div class="audit-detail">
              <div class="audit-checks">{#each report.checks as check}<div><span class="check-dot {check.status}">{check.status === "pass" ? "✓" : check.status === "fail" ? "!" : "·"}</span><strong>{check.name}</strong><small>{check.evidence}</small></div>{/each}</div>
              <div class="audit-advice"><h3>{tr(report.suggestions.length ? "audit.optimize" : "common.details", {}, $locale)}</h3>{#if report.suggestions.length}{#each report.suggestions as suggestion}<p><ShieldAlert size={14} />{suggestion}</p>{/each}{:else}<p><CheckCircle2 size={14} />{tr("audit.usable", {}, $locale)}</p>{/if}<button class="text-button" onclick={() => showAsset(report)}>{tr("common.details", {}, $locale)}</button></div>
            </div>
          {/if}
        </article>
      {/each}
    </div>
    {#if result.truncated}<p class="audit-footnote">{tr("common.items", { count: 240 }, $locale)} · {tr("audit.optimize", {}, $locale)}</p>{/if}
  {/if}
</section>
