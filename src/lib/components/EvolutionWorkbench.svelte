<script lang="ts">
  import { Bot, Check, ChevronLeft, ChevronRight, CircleAlert, RefreshCw, Search, ShieldCheck, Sparkles } from "@lucide/svelte";
  import type { AssetGroup, RefreshResult } from "../types";
  import { locale, tr } from "../i18n";

  export let groups: AssetGroup[];
  export let refreshing: boolean;
  export let refreshResult: RefreshResult | null;
  export let runRefresh: () => void;
  export let openAsset: (asset: AssetGroup) => void;

  type QueueFilter = "conflicts" | "unverified" | "license" | "quality" | "all";
  const PAGE_SIZE = 30;
  let queueFilter: QueueFilter = "conflicts";
  let kindFilter = "all";
  let sortBy = "risk";
  let queueQuery = "";
  let queuePage = 1;

  $: conflicts = groups.filter((asset) => asset.variants.length > 1);
  $: unverified = groups.filter((asset) => (asset.ready_versions?.length ?? 0) === 0);
  $: licensePending = groups.filter(hasPendingLicense);
  $: qualityPending = groups.filter(hasMetadataGap);
  $: readyCount = groups.length - unverified.length;
  $: attention = groups.filter((asset) => riskScore(asset) > 0);
  $: normalizedQuery = queueQuery.trim().toLocaleLowerCase();
  $: filteredQueue = attention
    .filter((asset) => {
      const categoryMatch =
        queueFilter === "all" ||
        (queueFilter === "conflicts" && asset.variants.length > 1) ||
        (queueFilter === "unverified" && (asset.ready_versions?.length ?? 0) === 0) ||
        (queueFilter === "license" && hasPendingLicense(asset)) ||
        (queueFilter === "quality" && hasMetadataGap(asset));
      return categoryMatch && (kindFilter === "all" || asset.kind.toLowerCase() === kindFilter) &&
        (!normalizedQuery || `${asset.name} ${asset.logical_id} ${asset.description ?? ""}`.toLocaleLowerCase().includes(normalizedQuery));
    })
    .sort((left, right) => {
      if (sortBy === "name") return left.name.localeCompare(right.name, $locale);
      if (sortBy === "versions") return right.variants.length - left.variants.length || left.name.localeCompare(right.name, $locale);
      return riskScore(right) - riskScore(left) || right.variants.length - left.variants.length || left.name.localeCompare(right.name, $locale);
    });
  $: totalPages = Math.max(1, Math.ceil(filteredQueue.length / PAGE_SIZE));
  $: currentPage = Math.min(queuePage, totalPages);
  $: visibleQueue = filteredQueue.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE);

  function hasPendingLicense(asset: AssetGroup): boolean {
    return !asset.license || asset.license.includes("待") || asset.license.includes("未知");
  }
  function metadataReasons(asset: AssetGroup): string[] {
    const description = (asset.description ?? "").trim();
    const usage = (asset.usage ?? "").trim();
    return [
      description.length < 40 ? tr("audit.optimize", {}, $locale) : "",
      usage.length < 30 ? tr("audit.optimize", {}, $locale) : "",
      description && description === usage ? tr("audit.optimize", {}, $locale) : ""
    ].filter(Boolean);
  }
  function hasMetadataGap(asset: AssetGroup): boolean {
    return metadataReasons(asset).length > 0;
  }
  function riskScore(asset: AssetGroup): number {
    return (asset.variants.length > 1 ? 4 : 0) + ((asset.ready_versions?.length ?? 0) === 0 ? 2 : 0) + (hasPendingLicense(asset) ? 1 : 0) + (hasMetadataGap(asset) ? 1 : 0);
  }
  function reasons(asset: AssetGroup): string {
    return [asset.variants.length > 1 ? `${asset.variants.length} ${tr("status.versionConflict", {}, $locale)}` : "", (asset.ready_versions?.length ?? 0) === 0 ? tr("assets.pendingVerification", {}, $locale) : "", hasPendingLicense(asset) ? tr("status.licensePending", {}, $locale) : "", ...metadataReasons(asset)].filter(Boolean).join(" · ");
  }
  function formatRate(value: number): string {
    if (!Number.isFinite(value) || value <= 0) return "0%";
    if (value < 1) return `${value.toFixed(2)}%`;
    if (value < 10) return `${value.toFixed(1)}%`;
    return `${Math.round(value)}%`;
  }
  function chooseQueue(filter: QueueFilter) {
    queueFilter = filter;
    queuePage = 1;
  }
</script>

<section class="compact-heading evolution-heading">
  <div><span class="eyebrow">VERSION & EVOLUTION</span><h1>{tr("evolution.title", {}, $locale)}</h1><p>{tr("evolution.subtitle", {}, $locale)}</p></div>
  <button class="secondary-button" disabled={refreshing} onclick={runRefresh}><RefreshCw size={16} /> {tr(refreshing ? "evolution.scanning" : "evolution.deepRefresh", {}, $locale)}</button>
</section>

<section class="evolution-summary" aria-label={tr("common.status", {}, $locale)}>
  <button class:active={queueFilter === "conflicts"} aria-pressed={queueFilter === "conflicts"} onclick={() => chooseQueue("conflicts")}><span>{tr("projects.attention", {}, $locale)}</span><strong>{conflicts.length}</strong><small>{tr("status.versionConflict", {}, $locale)}</small></button>
  <button class:active={queueFilter === "unverified"} aria-pressed={queueFilter === "unverified"} onclick={() => chooseQueue("unverified")}><span>{tr("drawer.verification", {}, $locale)}</span><strong>{unverified.length}</strong><small>{tr("assets.pendingVerification", {}, $locale)}</small></button>
  <button class:active={queueFilter === "license"} aria-pressed={queueFilter === "license"} onclick={() => chooseQueue("license")}><span>{tr("common.confirm", {}, $locale)}</span><strong>{licensePending.length}</strong><small>{tr("status.licensePending", {}, $locale)}</small></button>
  <button class:active={queueFilter === "quality"} aria-pressed={queueFilter === "quality"} onclick={() => chooseQueue("quality")}><span>{tr("audit.optimize", {}, $locale)}</span><strong>{qualityPending.length}</strong><small>{tr("audit.optimize", {}, $locale)}</small></button>
  <button class:active={queueFilter === "all"} aria-pressed={queueFilter === "all"} onclick={() => chooseQueue("all")}><span>{tr("common.all", {}, $locale)}</span><strong>{attention.length}</strong><small>{readyCount} Ready</small></button>
</section>

<section class="evolution-policy">
  <ShieldCheck size={20} />
  <div><strong>{tr("evolution.policy", {}, $locale)}</strong><span>{tr("evolution.policyBody", {}, $locale)}</span></div>
  <div class="readiness-meter"><span style={`width:${groups.length ? (readyCount / groups.length) * 100 : 0}%`}></span></div>
  <em>{formatRate(groups.length ? (readyCount / groups.length) * 100 : 0)} Ready</em>
</section>

<section class="evolution-queue panel">
  <div class="queue-toolbar">
    <div><span class="section-kicker">TRIAGE QUEUE</span><h2>{tr(queueFilter === "conflicts" ? "status.versionConflict" : queueFilter === "unverified" ? "assets.pendingVerification" : queueFilter === "license" ? "status.licensePending" : queueFilter === "quality" ? "audit.optimize" : "common.all", {}, $locale)}</h2></div>
    <label class="queue-search"><Search size={14} /><span class="sr-only">{tr("common.search", {}, $locale)}</span><input bind:value={queueQuery} oninput={() => (queuePage = 1)} placeholder={tr("common.search", {}, $locale)} /></label>
    <select bind:value={kindFilter} onchange={() => (queuePage = 1)} aria-label={tr("common.typeFilter", {}, $locale)}><option value="all">{tr("common.all", {}, $locale)}</option><option value="agent">Agent</option><option value="skill">Skill</option></select>
    <select bind:value={sortBy} onchange={() => (queuePage = 1)} aria-label={tr("common.sort", {}, $locale)}><option value="risk">{tr("projects.attention", {}, $locale)}</option><option value="versions">{tr("common.versions", {}, $locale)}</option><option value="name">{tr("common.assets", {}, $locale)}</option></select>
  </div>
  <div class="triage-head"><span>{tr("common.assets", {}, $locale)}</span><span>{tr("status.truthBoundary", {}, $locale)}</span><span>{tr("nav.projects", {}, $locale)}</span><span>{tr("common.status", {}, $locale)}</span></div>
  <div class="triage-list">
    {#each visibleQueue as asset (asset.logical_id)}
      <button onclick={() => openAsset(asset)}>
        <span class="asset-glyph {asset.kind.toLowerCase()}">{#if asset.kind.toLowerCase() === "agent"}<Bot size={15} />{:else}<Sparkles size={15} />{/if}</span>
        <span class="triage-name"><strong>{asset.name}</strong><small>{asset.kind} · {asset.logical_id}</small></span>
        <span class="triage-reasons">{reasons(asset)}</span>
        <span class="triage-projects">{(asset.projects ?? []).slice(0, 2).join(" · ") || tr("common.notLinked", {}, $locale)}</span>
        <em class:risk-high={riskScore(asset) >= 6}>{tr(riskScore(asset) >= 6 ? "projects.attention" : "audit.optimize", {}, $locale)}</em>
        <ChevronRight size={15} />
      </button>
    {/each}
    {#if visibleQueue.length === 0}<div class="triage-empty"><Check size={20} /><strong>{tr("common.none", {}, $locale)}</strong><span>{tr("assets.noMatches", {}, $locale)}</span></div>{/if}
  </div>
  <footer class="queue-pagination"><span>{tr("common.items", { count: filteredQueue.length }, $locale)} · {tr("common.page", { current: currentPage, total: totalPages }, $locale)}</span><div><button disabled={currentPage === 1} onclick={() => (queuePage = currentPage - 1)}><ChevronLeft size={14} />{tr("common.previous", {}, $locale)}</button><button disabled={currentPage === totalPages} onclick={() => (queuePage = currentPage + 1)}>{tr("common.next", {}, $locale)}<ChevronRight size={14} /></button></div></footer>
</section>

{#if refreshResult}
  <section class="refresh-result panel"><div class="panel-heading"><div><span class="section-kicker">LAST REFRESH</span><h2>{tr("common.refresh", {}, $locale)}</h2></div><span class="status-chip {refreshResult.status === 'success' ? 'good' : 'warn'}">{refreshResult.status}</span></div>{#each refreshResult.steps as step}<div class="refresh-step"><span class:ok={step.status === "success"} class:fail={step.status !== "success"}>{#if step.status === "success"}<Check size={13} />{:else}<CircleAlert size={13} />{/if}</span><strong>{step.name}</strong><small>{step.summary}</small></div>{/each}</section>
{/if}
