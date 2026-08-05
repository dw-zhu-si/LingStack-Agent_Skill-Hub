<script lang="ts">
  import {
    Bot,
    Check,
    ChevronLeft,
    ChevronRight,
    ChevronsLeft,
    ChevronsRight,
    Download,
    ShieldCheck,
    RotateCcw,
    Sparkles
  } from "@lucide/svelte";
  import type { AssetGovernanceRecord, AssetGroup } from "../types";
  import { formatLocaleNumber, locale, tr } from "../i18n";

  export let groups: AssetGroup[];
  export let query: string;
  export let selectedIds: Set<string>;
  export let toggleSelection: (id: string) => void;
  export let addSelection: (ids: string[]) => void;
  export let clearSelection: () => void;
  export let openAsset: (asset: AssetGroup) => void;
  export let governanceById: Map<string, AssetGovernanceRecord>;
  export let manageSelection: () => void;
  export let goToExports: () => void;

  const PAGE_SIZES = [50, 100, 200] as const;
  let kindFilter = "all";
  let stateFilter = "all";
  let sortBy = "name";
  let assetPage = 1;
  let assetPageSize = 100;
  let previousQuery = query;

  $: searchIndex = new Map(
    groups.map((asset) => [
      asset.logical_id,
      [asset.name, asset.description, asset.logical_id, ...(asset.tools ?? []), ...(asset.projects ?? [])]
        .filter(Boolean)
        .join(" ")
        .toLocaleLowerCase()
    ])
  );
  $: normalizedQuery = query.trim().toLocaleLowerCase();
  $: filteredGroups = groups.filter((asset) => {
    const matchesSearch = !normalizedQuery || searchIndex.get(asset.logical_id)?.includes(normalizedQuery);
    const matchesKind = kindFilter === "all" || asset.kind.toLowerCase() === kindFilter;
    const matchesState =
      stateFilter === "all" ||
      (stateFilter === "ready" && (asset.ready_versions?.length ?? 0) > 0) ||
      (stateFilter === "verified" && !!governanceById.get(asset.logical_id)?.verification) ||
      (stateFilter === "selected" && !!governanceById.get(asset.logical_id)?.selected_sha256) ||
      (stateFilter === "multi" && asset.variants.length > 1) ||
      (stateFilter === "pending" && (asset.ready_versions?.length ?? 0) === 0);
    return matchesSearch && matchesKind && matchesState;
  });
  $: sortedGroups = [...filteredGroups].sort((left, right) => {
    if (sortBy === "versions") {
      return right.variants.length - left.variants.length || left.name.localeCompare(right.name, $locale);
    }
    if (sortBy === "state") {
      return stateRank(left) - stateRank(right) || left.name.localeCompare(right.name, $locale);
    }
    return left.name.localeCompare(right.name, $locale);
  });
  $: totalAssetPages = Math.max(1, Math.ceil(sortedGroups.length / assetPageSize));
  $: currentAssetPage = Math.min(Math.max(assetPage, 1), totalAssetPages);
  $: pageStart = (currentAssetPage - 1) * assetPageSize;
  $: visibleGroups = sortedGroups.slice(pageStart, pageStart + assetPageSize);
  $: if (query !== previousQuery) {
    previousQuery = query;
    assetPage = 1;
  }

  function formatNumber(value: number): string {
    return formatLocaleNumber(value, $locale);
  }

  function lifecycleTone(asset: AssetGroup): "good" | "warn" | "neutral" | "danger" {
    const governance = governanceById.get(asset.logical_id);
    if (governance?.pending_refresh) return "warn";
    if (governance?.verification) return "good";
    if ((asset.ready_versions?.length ?? 0) > 0) return "good";
    if (asset.variants.length > 1) return "warn";
    if ((asset.lifecycle_state ?? "").includes("隔离")) return "danger";
    return "neutral";
  }

  function lifecycleLabel(asset: AssetGroup): string {
    const governance = governanceById.get(asset.logical_id);
    if (governance?.pending_refresh) return tr("assets.optimizedPending", {}, $locale);
    if (governance?.verification) return tr("assets.verified", {}, $locale);
    if ((asset.ready_versions?.length ?? 0) > 0) return tr("assets.ready", {}, $locale);
    if (governance?.selected_sha256 && asset.variants.length > 1) return tr("assets.selectedPending", {}, $locale);
    if (governance?.license) return tr("assets.licensePending", {}, $locale);
    if (asset.variants.length > 1) return tr("status.multiVersion", {}, $locale);
    return asset.lifecycle_state || tr("assets.visiblePending", {}, $locale);
  }

  function stateRank(asset: AssetGroup): number {
    if (asset.variants.length > 1) return 0;
    if ((asset.ready_versions?.length ?? 0) === 0) return 1;
    return 2;
  }

  function resetPage() {
    assetPage = 1;
  }

  function resetFilters() {
    kindFilter = "all";
    stateFilter = "all";
    sortBy = "name";
    resetPage();
  }
</script>

<section class="compact-heading asset-heading">
  <div>
    <span class="eyebrow">ASSET EXPLORER</span>
    <h1>{tr("assets.title", {}, $locale)}</h1>
    <p>{tr("assets.subtitle", { count: formatNumber(sortedGroups.length) }, $locale)}</p>
  </div>
  <div class="asset-heading-actions">
    <button class="secondary-button small" disabled={visibleGroups.length === 0} onclick={() => addSelection(visibleGroups.map((asset) => asset.logical_id))}>
      {tr("common.selectPage", {}, $locale)}
    </button>
    <button class="secondary-button small" disabled={sortedGroups.length === 0} onclick={() => addSelection(sortedGroups.map((asset) => asset.logical_id))}>
      {tr("common.selectAllMatches", {}, $locale)}
    </button>
  </div>
</section>

<section class="filterbar">
  <div class="segmented" aria-label={tr("common.typeFilter", {}, $locale)}>
    {#each [{id:"all", label:tr("common.all", {}, $locale)}, {id:"agent", label:"Agent"}, {id:"skill", label:"Skill"}] as filter}
      <button
        class:active={kindFilter === filter.id}
        onclick={() => {
          kindFilter = filter.id;
          resetPage();
        }}
      >
        {filter.label}
      </button>
    {/each}
  </div>
  <select bind:value={stateFilter} aria-label={tr("common.stateFilter", {}, $locale)} onchange={resetPage}>
    <option value="all">{tr("common.all", {}, $locale)}</option>
    <option value="ready">{tr("assets.ready", {}, $locale)}</option>
    <option value="verified">{tr("assets.verified", {}, $locale)}</option>
    <option value="selected">{tr("assets.versionSelected", {}, $locale)}</option>
    <option value="multi">{tr("assets.multiVersion", {}, $locale)}</option>
    <option value="pending">{tr("assets.pendingVerification", {}, $locale)}</option>
  </select>
  <select bind:value={sortBy} aria-label={tr("common.sort", {}, $locale)} onchange={resetPage}>
    <option value="name">{tr("common.sort", {}, $locale)} · {tr("assets.title", {}, $locale)}</option>
    <option value="versions">{tr("common.versions", {}, $locale)}</option>
    <option value="state">{tr("projects.attention", {}, $locale)}</option>
  </select>
  {#if kindFilter !== "all" || stateFilter !== "all" || sortBy !== "name"}
    <button class="text-button" onclick={resetFilters}><RotateCcw size={13} />{tr("common.reset", {}, $locale)}</button>
  {/if}
  <span class="result-count">{tr("common.items", { count: selectedIds.size }, $locale)} {tr("common.selected", {}, $locale)}</span>
  {#if selectedIds.size > 0}
    <button class="text-button" onclick={clearSelection}>{tr("common.clear", {}, $locale)}</button>
    <button class="primary-button small" onclick={goToExports}>
      <Download size={15} /> {tr("common.export", {}, $locale)}
    </button>
    <button class="primary-button small" onclick={manageSelection}>
      <ShieldCheck size={15} /> {tr("common.manageSelected", {}, $locale)}
    </button>
  {/if}
</section>

<section class="asset-table panel">
  <div class="table-head">
    <span></span><span>{tr("common.assets", {}, $locale)}</span><span>{tr("common.toolsProjects", {}, $locale)}</span><span>{tr("common.versions", {}, $locale)}</span><span>{tr("common.status", {}, $locale)}</span><span></span>
  </div>
  <div class="table-body">
    {#each visibleGroups as asset (asset.logical_id)}
      <div class="table-row">
        <label class="checkbox" aria-label={`${tr("common.selected", {}, $locale)} ${asset.name}`}>
          <input
            type="checkbox"
            checked={selectedIds.has(asset.logical_id)}
            onchange={() => toggleSelection(asset.logical_id)}
          />
          <span><Check size={12} /></span>
        </label>
        <button class="asset-cell" onclick={() => openAsset(asset)}>
          <span class="asset-glyph {asset.kind.toLowerCase()}">
            {#if asset.kind.toLowerCase() === "agent"}<Bot size={17} />{:else}<Sparkles size={17} />{/if}
          </span>
          <span>
            <strong>{asset.name}</strong>
            <small>{asset.kind} · {asset.description || asset.logical_id}</small>
          </span>
        </button>
        <div class="meta-cell">
          <strong>{(asset.tools ?? []).slice(0, 2).join(" · ") || tr("common.none", {}, $locale)}</strong>
          <small>{(asset.projects ?? []).slice(0, 2).join(" · ") || tr("common.notLinked", {}, $locale)}</small>
        </div>
        <div class="version-cell">
          <strong>{asset.variants.length}</strong>
          <small>{tr("drawer.versions", {}, $locale)}</small>
        </div>
        <span class="status-chip {lifecycleTone(asset)}">{lifecycleLabel(asset)}</span>
        <button class="row-action" aria-label={`${tr("common.details", {}, $locale)} ${asset.name}`} onclick={() => openAsset(asset)}>
          <ChevronRight size={17} />
        </button>
      </div>
    {/each}
    {#if sortedGroups.length === 0}
      <div class="empty-table">{tr("assets.noMatches", {}, $locale)}</div>
    {/if}
  </div>
  {#if sortedGroups.length > 0}
    <footer class="table-pagination" aria-label={tr("common.page", { current: currentAssetPage, total: totalAssetPages }, $locale)}>
      <span>
        显示 {formatNumber(pageStart + 1)}–{formatNumber(Math.min(pageStart + assetPageSize, sortedGroups.length))}
        / {formatNumber(sortedGroups.length)}
      </span>
      <label>
        {tr("common.page", { current: currentAssetPage, total: totalAssetPages }, $locale)}
        <select bind:value={assetPageSize} onchange={resetPage} aria-label={tr("common.items", { count: assetPageSize }, $locale)}>
          {#each PAGE_SIZES as size}
            <option value={size}>{size}</option>
          {/each}
        </select>
      </label>
      <div class="page-controls">
        <button disabled={currentAssetPage === 1} onclick={() => (assetPage = 1)} aria-label={tr("common.previous", {}, $locale)}><ChevronsLeft size={14} /></button>
        <button disabled={currentAssetPage === 1} onclick={() => (assetPage = currentAssetPage - 1)} aria-label={tr("common.previous", {}, $locale)}><ChevronLeft size={14} /></button>
        <strong>{currentAssetPage} / {totalAssetPages}</strong>
        <button disabled={currentAssetPage === totalAssetPages} onclick={() => (assetPage = currentAssetPage + 1)} aria-label={tr("common.next", {}, $locale)}><ChevronRight size={14} /></button>
        <button disabled={currentAssetPage === totalAssetPages} onclick={() => (assetPage = totalAssetPages)} aria-label={tr("common.next", {}, $locale)}><ChevronsRight size={14} /></button>
      </div>
    </footer>
  {/if}
</section>
