<script lang="ts">
  import { onMount, tick } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    Archive,
    ArrowUpRight,
    Bot,
    Boxes,
    Check,
    ChevronRight,
    CircleAlert,
    Download,
    FolderKanban,
    Gauge,
    Globe2,
    Layers3,
    Menu,
    PackageCheck,
    RefreshCw,
    Search,
    Settings2,
    ShieldCheck,
    Sparkles,
    WandSparkles,
    X
  } from "@lucide/svelte";
  import ExportCenter from "./lib/components/ExportCenter.svelte";
  import DefinitionPreview from "./lib/components/DefinitionPreview.svelte";
  import VariantComparison from "./lib/components/VariantComparison.svelte";
  import { hasValidVerification, assetVersionKey } from "./lib/assetState";
  import AssetExplorer from "./lib/components/AssetExplorer.svelte";
  import AssetGovernancePanel from "./lib/components/AssetGovernancePanel.svelte";
  import EvolutionWorkbench from "./lib/components/EvolutionWorkbench.svelte";
  import ProjectAtlas from "./lib/components/ProjectAtlas.svelte";
  import UnifiedControlCenter from "./lib/components/UnifiedControlCenter.svelte";
  import {
    applyDocumentLocale,
    formatLocaleNumber,
    locale,
    localeOptions,
    setLocale,
    tr,
    type LocaleCode
  } from "./lib/i18n";
  import {
    exportAssets,
    inspectInventory,
    loadAssetGovernance,
    loadInventory,
    refreshInventory,
    resolveAgentSkills
  } from "./lib/api";
  import type {
    AgentSkillResolution,
    AssetGovernanceRecord,
    AssetGroup,
    ExportFormat,
    ExportHistoryEntry,
    ExportMode,
    ExportResult,
    InventoryEnvelope,
    ProjectSummary,
    RefreshResult,
    ViewId
  } from "./lib/types";

  const navItems: Array<{ id: ViewId; labelKey: string; icon: typeof Gauge }> = [
    { id: "overview", labelKey: "nav.overview", icon: Gauge },
    { id: "assets", labelKey: "nav.assets", icon: Boxes },
    { id: "projects", labelKey: "nav.projects", icon: FolderKanban },
    { id: "updates", labelKey: "nav.evolution", icon: WandSparkles },
    { id: "controls", labelKey: "nav.controls", icon: Settings2 },
    { id: "exports", labelKey: "nav.exports", icon: Archive }
  ];
  const AUTO_CHECK_INTERVAL_MS = 60_000;
  const EXPORT_HISTORY_KEY = "lingzhan.export-history.v1";
  const EXPORT_HISTORY_LIMIT = 20;
  const PUBLIC_RELEASE = import.meta.env.VITE_LINGZHAN_PUBLIC_RELEASE !== "0";

  let currentView: ViewId = "overview";
  let envelope: InventoryEnvelope | null = null;
  let loading = true;
  let error = "";
  let query = "";
  let projectFilter = "";
  let selectedAsset: AssetGroup | null = null;
  let selectedDefinitionHash = "";
  let candidateRequest = 0;
  let selectedIds = new Set<string>();
  let refreshing = false;
  let refreshResult: RefreshResult | null = null;
  let sidebarOpen = false;
  let exportFormat: ExportFormat = "json";
  let exportMode: ExportMode = "selection";
  let exporting = false;
  let analyzingRelations = false;
  let relationPreview: AgentSkillResolution | null = null;
  let toast = "";
  let lastAutoCheck = "";
  let searchInput: HTMLInputElement;
  let inventoryLoading = false;
  let checkingSource = false;
  let exportHistory: ExportHistoryEntry[] = [];
  let governanceRecords: AssetGovernanceRecord[] = [];

  $: registryGroups = envelope?.registry.groups ?? [];
  $: governanceById = new Map(governanceRecords.map((record) => [record.logical_id, record]));
  $: groups = registryGroups.filter((asset) => {
    const kind = asset.kind.toLowerCase();
    return kind === "agent" || kind === "skill";
  });
  $: agents = groups.filter((asset) => asset.kind.toLowerCase() === "agent");
  $: skills = groups.filter((asset) => asset.kind.toLowerCase() === "skill");
  $: supportRecordCount = registryGroups.length - groups.length;
  $: physicalAssetCount = groups.reduce((total, asset) => total + asset.variants.length, 0);
  $: uniqueAssetHashCount = new Set(
    groups.flatMap((asset) => asset.variants.map((variant) => variant.sha256).filter(Boolean))
  ).size;
  $: selectedGroups = groups.filter((asset) => selectedIds.has(asset.logical_id));
  $: selectedAgents = selectedGroups.filter((asset) => asset.kind.toLowerCase() === "agent");
  $: selectedSkills = selectedGroups.filter((asset) => asset.kind.toLowerCase() === "skill");
  $: readyAssets = groups.filter((asset) => (asset.ready_versions?.length ?? 0) > 0);
  $: multiVersionAssets = groups.filter((asset) => asset.variants.length > 1);
  $: pendingLicenseAssets = groups.filter(
    (asset) => !asset.license || asset.license.includes("待") || asset.license.includes("未知")
  );
  $: readyRate = groups.length > 0 ? (readyAssets.length / groups.length) * 100 : 0;
  $: attentionAssets = groups.filter(
    (asset) =>
      asset.variants.length > 1 ||
      (asset.ready_versions?.length ?? 0) === 0 ||
      !asset.license ||
      asset.license.includes("待")
  );
  $: projectRows = buildProjectRows(groups);
  $: if (relationPreview && relationPreview.agent_logical_id !== selectedAgents[0]?.logical_id) {
    relationPreview = null;
  }

  function buildProjectRows(assets: AssetGroup[]): ProjectSummary[] {
    const projects = new Map<string, ProjectSummary>();
    for (const asset of assets) {
      for (const project of asset.projects ?? []) {
        if (!project) continue;
        const row = projects.get(project) ?? {
          name: project,
          agents: 0,
          skills: 0,
          assets: 0,
          ready: 0
        };
        row.assets += 1;
        if (asset.kind.toLowerCase() === "agent") row.agents += 1;
        if (asset.kind.toLowerCase() === "skill") row.skills += 1;
        if ((asset.ready_versions?.length ?? 0) > 0) row.ready += 1;
        projects.set(project, row);
      }
    }
    return [...projects.values()].sort((a, b) => b.assets - a.assets || a.name.localeCompare(b.name));
  }

  function formatNumber(value: number): string {
    return formatLocaleNumber(value, $locale);
  }

  function formatRate(value: number): string {
    if (!Number.isFinite(value) || value <= 0) return "0%";
    if (value < 1) return `${value.toFixed(2)}%`;
    if (value < 10) return `${value.toFixed(1)}%`;
    return `${Math.round(value)}%`;
  }

  function compactHash(value?: string): string {
    if (!value) return "—";
    return `${value.slice(0, 8)}…${value.slice(-6)}`;
  }

  function lifecycleTone(asset: AssetGroup): "good" | "warn" | "neutral" | "danger" {
    const governance = governanceById.get(asset.logical_id);
    if (governance?.pending_refresh) return "warn";
    if (hasValidVerification(asset, governance)) return "good";
    if ((asset.ready_versions?.length ?? 0) > 0) return "good";
    if (asset.variants.length > 1) return "warn";
    if ((asset.lifecycle_state ?? "").includes("隔离")) return "danger";
    return "neutral";
  }

  function lifecycleLabel(asset: AssetGroup): string {
    const governance = governanceById.get(asset.logical_id);
    if (governance?.pending_refresh) return tr("assets.optimizedPending", {}, $locale);
    if (hasValidVerification(asset, governance)) return tr("assets.verified", {}, $locale);
    if ((asset.ready_versions?.length ?? 0) > 0) return tr("assets.ready", {}, $locale);
    if (governance?.selected_sha256 && asset.variants.length > 1) return tr("assets.selectedPending", {}, $locale);
    if (governance?.license) return tr("assets.licensePending", {}, $locale);
    if (asset.variants.length > 1) return tr("status.multiVersion", {}, $locale);
    return asset.lifecycle_state || tr("assets.visiblePending", {}, $locale);
  }

  function toggleSelection(id: string) {
    const next = new Set(selectedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selectedIds = next;
  }

  function addSelection(ids: string[]) {
    selectedIds = new Set([...selectedIds, ...ids]);
  }

  function showProjectAssets(project: string) {
    query = "";
    projectFilter = project;
    goTo("assets");
  }

  function clearSelection() {
    selectedIds = new Set();
  }

  function showToast(message: string) {
    toast = message;
    window.setTimeout(() => {
      if (toast === message) toast = "";
    }, 4200);
  }

  function reportError(reason: unknown) {
    error = reason instanceof Error ? reason.message : String(reason);
  }

  async function fetchGovernance() {
    try { governanceRecords = await loadAssetGovernance(); }
    catch (reason) { reportError(reason); }
  }

  function updateGovernance(record: AssetGovernanceRecord) {
    const index = governanceRecords.findIndex((item) => item.logical_id === record.logical_id);
    governanceRecords = index < 0
      ? [...governanceRecords, record]
      : governanceRecords.map((item, itemIndex) => itemIndex === index ? record : item);
    showToast(tr(record.pending_refresh ? "assets.optimizedPending" : "common.status", {}, $locale));
  }

  function manageSelection() {
    const candidate = selectedGroups.find((asset) => !hasValidVerification(asset, governanceById.get(asset.logical_id))) ?? selectedGroups[0];
    if (candidate) {
      openAsset(candidate);
      showToast(`已打开“${candidate.name}”的认证/选版/优化工作流`);
    }
  }

  async function fetchInventory(silent = false) {
    if (inventoryLoading) return;
    inventoryLoading = true;
    if (!silent) loading = true;
    try {
      const next = await loadInventory();
      if (silent && envelope?.source_fingerprint === next.source_fingerprint) {
        lastAutoCheck = new Date().toLocaleTimeString($locale, { hour: "2-digit", minute: "2-digit" });
        return;
      }
      envelope = next;
      const assetsById = new Map(next.registry.groups.map((asset) => [asset.logical_id, asset]));
      const validSelection = [...selectedIds].filter((id) => assetsById.has(id));
      if (validSelection.length !== selectedIds.size) selectedIds = new Set(validSelection);
      if (selectedAsset) selectedAsset = assetsById.get(selectedAsset.logical_id) ?? null;
      error = "";
      lastAutoCheck = new Date().toLocaleTimeString($locale, { hour: "2-digit", minute: "2-digit" });
      if (silent) showToast(tr("common.refresh", {}, $locale));
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      loading = false;
      inventoryLoading = false;
    }
  }

  async function checkForInventoryUpdate() {
    if (document.visibilityState !== "visible" || refreshing || checkingSource) return;
    checkingSource = true;
    try {
      const source = await inspectInventory();
      lastAutoCheck = new Date().toLocaleTimeString($locale, { hour: "2-digit", minute: "2-digit" });
      if (!envelope || source.source_fingerprint !== envelope.source_fingerprint) {
        await fetchInventory(true);
      }
    } catch (reason) {
      if (!envelope) error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      checkingSource = false;
    }
  }

  async function runRefresh(mode: "local" | "full") {
    refreshing = true;
    refreshResult = null;
    try {
      const result = await refreshInventory(mode);
      refreshResult = result;
      await fetchInventory(true);
      await fetchGovernance();
      if (refreshResult && envelope) {
        refreshResult = { ...refreshResult, registry_updated_at: envelope.registry.updated_at };
      }
      showToast(
        result.status === "success"
          ? tr(mode === "local" ? "overview.refreshLocal" : "evolution.deepRefresh", {}, $locale)
          : tr("audit.blocked", {}, $locale)
      );
      if (result.status === "blocked") goTo("updates");
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      refreshing = false;
    }
  }

  async function runExport(ids: string[]) {
    if (ids.length === 0) {
      showToast(tr("common.selected", {}, $locale));
      return;
    }
    if (exportMode === "agents_only" && selectedAgents.length === 0) {
      showToast(`Agent: ${tr("common.none", {}, $locale)}`);
      return;
    }
    if (exportMode === "skills_only" && selectedSkills.length === 0) {
      showToast(`Skill: ${tr("common.none", {}, $locale)}`);
      return;
    }
    if (exportMode === "agent_with_skills" && selectedAgents.length !== 1) {
      showToast(`Agent: 1`);
      return;
    }
    try {
      const extension = exportFormat === "bundle" ? "zip" : exportFormat === "markdown" ? "md" : "json";
      const prefix =
        exportMode === "agents_only"
          ? "agent-export"
          : exportMode === "skills_only"
            ? "skill-export"
            : exportMode === "agent_with_skills"
              ? "agent-skill-loadout"
              : "agent-skill-export";
      const destination = await save({
        title: tr("export.title", {}, $locale),
        defaultPath: `${prefix}-${new Date().toISOString().slice(0, 10)}.${extension}`,
        filters: [{ name: exportFormat, extensions: [extension] }]
      });
      if (!destination) return;

      exporting = true;
      const result = await exportAssets(ids, exportFormat, destination, exportMode);
      recordExportResult(result);
      showToast(
        `已导出 ${result.agent_count} Agent / ${result.skill_count} Skill，SHA-256 ${result.sha256.slice(0, 10)}…`
      );
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      exporting = false;
    }
  }

  function recordExportResult(result: ExportResult) {
    exportHistory = [
      { ...result, completed_at: new Date().toISOString() },
      ...exportHistory
    ].slice(0, EXPORT_HISTORY_LIMIT);
    try {
      window.localStorage.setItem(EXPORT_HISTORY_KEY, JSON.stringify(exportHistory));
    } catch {
      // The export itself succeeded; unavailable local receipt storage must not turn it into a failure.
    }
  }

  function loadExportHistory() {
    try {
      const stored = JSON.parse(window.localStorage.getItem(EXPORT_HISTORY_KEY) ?? "[]");
      if (!Array.isArray(stored)) return;
      exportHistory = stored
        .filter(
          (entry): entry is ExportHistoryEntry =>
            typeof entry === "object" &&
            entry !== null &&
            typeof entry.completed_at === "string" &&
            typeof entry.destination === "string" &&
            typeof entry.sha256 === "string"
        )
        .slice(0, EXPORT_HISTORY_LIMIT);
    } catch {
      try {
        window.localStorage.removeItem(EXPORT_HISTORY_KEY);
      } catch {
        // Ignore unavailable or read-only WebView storage.
      }
    }
  }

  function clearExportHistory() {
    exportHistory = [];
    try {
      window.localStorage.removeItem(EXPORT_HISTORY_KEY);
    } catch {
      // The in-memory history is already cleared.
    }
  }

  async function analyzeCompanions() {
    relationPreview = null;
    if (selectedAgents.length !== 1) {
      showToast(`Agent: 1`);
      return;
    }
    analyzingRelations = true;
    try {
      relationPreview = await resolveAgentSkills(selectedAgents[0].logical_id);
      if (relationPreview.skills.length === 0) {
        showToast(`Skill: ${tr("common.notLinked", {}, $locale)}`);
      }
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      analyzingRelations = false;
    }
  }

  function chooseExportMode(mode: ExportMode) {
    exportMode = mode;
    relationPreview = null;
  }

  function openSingleExport(asset: AssetGroup, mode: ExportMode) {
    selectedIds = new Set([asset.logical_id]);
    chooseExportMode(mode);
    currentView = "exports";
    selectedAsset = null;
    if (mode === "agent_with_skills") {
      window.setTimeout(() => analyzeCompanions(), 0);
    }
  }

  function plannedAssetCount(): number {
    if (exportMode === "agents_only") return selectedAgents.length;
    if (exportMode === "skills_only") return selectedSkills.length;
    if (exportMode !== "agent_with_skills") return selectedGroups.length;
    const ids = new Set([
      ...selectedAgents.map((asset) => asset.logical_id),
      ...selectedSkills.map((asset) => asset.logical_id),
      ...(relationPreview?.agent_logical_id === selectedAgents[0]?.logical_id
        ? relationPreview.skills.map((skill) => skill.logical_id)
        : [])
    ]);
    return ids.size;
  }

  function exportReady(): boolean {
    if (exporting || selectedIds.size === 0) return false;
    if (exportMode === "agents_only") return selectedAgents.length > 0;
    if (exportMode === "skills_only") return selectedSkills.length > 0;
    if (exportMode === "agent_with_skills") return selectedAgents.length === 1;
    return true;
  }

  function openAsset(asset: AssetGroup, preferredHash = "") {
    selectedDefinitionHash = preferredHash;
    selectedAsset = asset;
  }

  function goTo(view: ViewId) {
    currentView = view;
    sidebarOpen = false;
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      goTo("assets");
      searchInput?.focus();
      searchInput?.select();
      return;
    }
    if (event.key === "Escape") {
      selectedAsset = null;
      sidebarOpen = false;
    }
  }

  onMount(() => {
    const unsubscribeLocale = locale.subscribe(applyDocumentLocale);
    loadExportHistory();
    fetchInventory();
    fetchGovernance();
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") checkForInventoryUpdate();
    };
    window.addEventListener("keydown", handleGlobalKeydown);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    const timer = window.setInterval(checkForInventoryUpdate, AUTO_CHECK_INTERVAL_MS);
    return () => {
      window.clearInterval(timer);
      unsubscribeLocale();
      window.removeEventListener("keydown", handleGlobalKeydown);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  });
</script>

<svelte:head>
  <title>{tr("app.title", {}, $locale)}</title>
</svelte:head>

<div class="app-shell">
  <a class="skip-link" href="#main-content">{tr("common.skipContent", {}, $locale)}</a>
  <button class="mobile-menu" aria-label={tr("common.openNavigation", {}, $locale)} onclick={() => (sidebarOpen = true)}>
    <Menu size={20} />
  </button>

  <aside class:open={sidebarOpen} class="sidebar">
    <div class="brand">
      <div class="brand-mark" aria-hidden="true">
        <Layers3 size={21} strokeWidth={2.2} />
      </div>
      <div>
        <strong>{tr("app.brand", {}, $locale)}</strong>
        <span>Agent & Skill Hub</span>
      </div>
      <button class="sidebar-close" aria-label={tr("common.closeNavigation", {}, $locale)} onclick={() => (sidebarOpen = false)}>
        <X size={18} />
      </button>
    </div>

    <nav aria-label={tr("common.mainNavigation", {}, $locale)}>
      <span class="nav-kicker">{tr("nav.workbench", {}, $locale)}</span>
      {#each navItems as item}
        <button
          class:active={currentView === item.id}
          class="nav-item"
          onclick={() => goTo(item.id)}
        >
          <item.icon size={18} />
          <span>{tr(item.labelKey, {}, $locale)}</span>
          {#if item.id === "updates" && multiVersionAssets.length > 0}
            <em>{multiVersionAssets.length}</em>
          {/if}
        </button>
      {/each}
    </nav>

    <div class="sidebar-footer">
      <div class="source-health">
        <span class="pulse"></span>
        <div>
          <strong>{tr(PUBLIC_RELEASE ? "source.private" : "source.local", {}, $locale)}</strong>
          <small>{tr(envelope ? "source.watching" : "source.connecting", {}, $locale)}</small>
        </div>
      </div>
      <div class="source-meta">
        <span>{tr("source.lastCheck", {}, $locale)}</span>
        <strong>{lastAutoCheck || "—"}</strong>
      </div>
      <div class="sidebar-legal">
        <button onclick={() => openUrl("https://pm.jcm99.com/apple/lingstack/privacy.html")}>
          {tr("common.privacyPolicy", {}, $locale)}
        </button>
        <span aria-hidden="true">·</span>
        <button onclick={() => openUrl("https://github.com/dw-zhu-si/LingStack-Agent_Skill-Hub/blob/main/SUPPORT.md")}>
          {tr("common.support", {}, $locale)}
        </button>
      </div>
    </div>
  </aside>

  {#if sidebarOpen}
    <button class="sidebar-backdrop" aria-label={tr("common.closeNavigation", {}, $locale)} onclick={() => (sidebarOpen = false)}></button>
  {/if}

  <main id="main-content">
    <header class="topbar">
      <div class="search-box">
        <Search size={18} aria-hidden="true" />
        <input
          bind:this={searchInput}
          bind:value={query}
          onfocus={() => currentView !== "assets" && (currentView = "assets")}
          autocomplete="off"
          spellcheck="false"
          placeholder={tr("search.placeholder", {}, $locale)}
          aria-label={tr("common.searchAssets", {}, $locale)}
        />
        {#if query}
          <button class="search-clear" aria-label={tr("common.clearSearch", {}, $locale)} title={tr("common.clearSearch", {}, $locale)} onclick={() => {
            query = "";
            searchInput?.focus();
          }}><X size={14} /></button>
        {:else}
          <kbd>⌘ K</kbd>
        {/if}
      </div>
      <div class="top-actions">
        <label class="locale-picker" title={tr("language.label", {}, $locale)}>
          <Globe2 size={16} aria-hidden="true" />
          <span class="sr-only">{tr("language.label", {}, $locale)}</span>
          <select
            aria-label={tr("language.label", {}, $locale)}
            value={$locale}
            onchange={(event) => setLocale(event.currentTarget.value as LocaleCode)}
          >
            {#each localeOptions as option}
              <option value={option.code}>{option.nativeName}</option>
            {/each}
          </select>
        </label>
        <div class="sync-state">
          <span class="sync-dot"></span>
          <span>{tr("common.registry", {}, $locale)} {envelope?.registry.updated_at ?? tr("common.notLoaded", {}, $locale)}</span>
        </div>
        <button
          class="icon-button"
          class:spinning={refreshing}
          aria-label={tr("overview.refreshLocal", {}, $locale)}
          title={tr("overview.refreshLocal", {}, $locale)}
          disabled={refreshing}
          onclick={() => runRefresh("local")}
        >
          <RefreshCw size={18} />
        </button>
        <button class="avatar" aria-label={tr("common.localUser", {}, $locale)}>{tr("common.localUserInitial", {}, $locale)}</button>
      </div>
    </header>

    {#if PUBLIC_RELEASE}
      <section class="release-privacy-banner" role="status">
        <ShieldCheck size={17} />
        <span><strong>{tr("release.bannerTitle", {}, $locale)}</strong> · {tr("release.bannerBody", {}, $locale)}</span>
      </section>
    {/if}

    {#if loading}
      <section class="loading-state">
        <div class="loader-orbit"><span></span></div>
        <h1>{tr("loading.title", {}, $locale)}</h1>
        <p>{tr("loading.body", {}, $locale)}</p>
      </section>
    {:else if error && !envelope}
      <section class="error-state">
        <CircleAlert size={32} />
        <h1>{tr("error.registryTitle", {}, $locale)}</h1>
        <p>{error}</p>
        <button class="primary-button" onclick={() => fetchInventory()}>{tr("error.reconnect", {}, $locale)}</button>
      </section>
    {:else if envelope}
      <div class="content">
        {#if currentView === "overview"}
          <section class="page-heading">
            <div>
              <span class="eyebrow">{tr("overview.eyebrow", {}, $locale)}</span>
              <h1>{tr("overview.title", {}, $locale)}</h1>
              <p>{tr("overview.subtitle", {}, $locale)}</p>
            </div>
            <button class="primary-button" disabled={refreshing} onclick={() => runRefresh("local")}>
              <RefreshCw size={17} />
              {tr(refreshing ? "overview.refreshing" : "overview.refreshLocal", {}, $locale)}
            </button>
          </section>

          <section class="metric-grid" aria-label={tr("common.assetStatistics", {}, $locale)}>
            <article class="metric-card metric-primary">
              <div class="metric-icon"><Boxes size={20} /></div>
              <div class="metric-label">{tr("metric.assets", {}, $locale)}</div>
              <strong>{formatNumber(groups.length)}</strong>
              <span>{tr("metric.definitions", { definitions: formatNumber(physicalAssetCount), support: formatNumber(supportRecordCount) }, $locale)}</span>
              <div class="metric-spark">
                <i style="height: 34%"></i><i style="height: 48%"></i><i style="height: 42%"></i
                ><i style="height: 67%"></i><i style="height: 61%"></i><i style="height: 86%"></i
                ><i style="height: 100%"></i>
              </div>
            </article>
            <article class="metric-card">
              <div class="metric-icon agent"><Bot size={20} /></div>
              <div class="metric-label">Agent</div>
              <strong>{formatNumber(agents.length)}</strong>
              <span>{tr("metric.toolSurfaces", { count: formatNumber(new Set(agents.flatMap((asset) => asset.tools ?? [])).size) }, $locale)}</span>
            </article>
            <article class="metric-card">
              <div class="metric-icon skill"><Sparkles size={20} /></div>
              <div class="metric-label">Skill</div>
              <strong>{formatNumber(skills.length)}</strong>
              <span>{tr("metric.uniqueHashes", { count: formatNumber(uniqueAssetHashCount) }, $locale)}</span>
            </article>
            <article class="metric-card">
              <div class="metric-icon ready"><PackageCheck size={20} /></div>
              <div class="metric-label">{tr("metric.reusable", {}, $locale)}</div>
              <strong>{formatNumber(readyAssets.length)}</strong>
              <span>{tr("metric.reusableEvidence", {}, $locale)}</span>
            </article>
          </section>

          <section class="dashboard-grid">
            <article class="panel attention-panel">
              <div class="panel-heading">
                <div>
                  <span class="section-kicker">{tr("status.pendingDecision", {}, $locale)}</span>
                  <h2>{tr("status.versionTrustQueue", {}, $locale)}</h2>
                </div>
                <button class="text-button" onclick={() => goTo("updates")}>
                  {tr("common.viewAll", {}, $locale)} <ArrowUpRight size={15} />
                </button>
              </div>
              <div class="attention-summary">
                <div>
                  <strong>{multiVersionAssets.length}</strong>
                  <span>{tr("status.multiVersion", {}, $locale)}</span>
                </div>
                <div>
                  <strong>{pendingLicenseAssets.length}</strong>
                  <span>{tr("status.licensePending", {}, $locale)}</span>
                </div>
                <div>
                  <strong>{groups.length - readyAssets.length}</strong>
                  <span>{tr("status.notPromoted", {}, $locale)}</span>
                </div>
              </div>
              <div class="attention-list">
                {#each attentionAssets.slice(0, 5) as asset}
                  <button onclick={() => openAsset(asset)}>
                    <span class="asset-glyph {asset.kind.toLowerCase()}">
                      {#if asset.kind.toLowerCase() === "agent"}<Bot size={16} />{:else}<Sparkles size={16} />{/if}
                    </span>
                    <span class="attention-copy">
                      <strong>{asset.name}</strong>
                      <small>
                        {asset.variants.length > 1
                          ? `${asset.variants.length} 个内容版本等待选择`
                          : asset.license || tr("status.licensePending", {}, $locale)}
                      </small>
                    </span>
                    <span class="status-chip {lifecycleTone(asset)}">{lifecycleLabel(asset)}</span>
                    <ChevronRight size={16} />
                  </button>
                {/each}
              </div>
            </article>

            <article class="panel trust-panel">
              <div class="panel-heading">
                <div>
                  <span class="section-kicker">{tr("status.truthBoundary", {}, $locale)}</span>
                  <h2>{tr("status.maturity", {}, $locale)}</h2>
                </div>
                <ShieldCheck size={21} />
              </div>
              <div class="readiness-signal" aria-label={`${tr("assets.ready", {}, $locale)} ${readyAssets.length}, ${tr("common.logicalAssets", { count: groups.length }, $locale)}, ${tr("status.totalCoverage", {}, $locale)} ${formatRate(readyRate)}`}>
                <div class="readiness-numbers">
                  <div>
                    <strong>{formatNumber(readyAssets.length)}</strong>
                    <span>{tr("status.readyAssets", {}, $locale)}</span>
                  </div>
                  <div class="readiness-rate">
                    <strong>{formatRate(readyRate)}</strong>
                    <span>{tr("status.totalCoverage", {}, $locale)}</span>
                  </div>
                </div>
                <div class="readiness-track" aria-hidden="true">
                  <span class:has-value={readyRate > 0} style={`--ready-rate:${readyRate}`}></span>
                </div>
                <div class="readiness-scale" aria-hidden="true">
                  <span>0</span>
                  <span>{tr("common.logicalAssets", { count: formatNumber(groups.length) }, $locale)}</span>
                </div>
              </div>
              <ul class="legend">
                <li><span class="legend-dot ready"></span>{tr("metric.reusable", {}, $locale)} <strong>{readyAssets.length}</strong></li>
                <li><span class="legend-dot visible"></span>{tr("status.visible", {}, $locale)} <strong>{groups.length - readyAssets.length}</strong></li>
                <li><span class="legend-dot version"></span>{tr("status.versionConflict", {}, $locale)} <strong>{multiVersionAssets.length}</strong></li>
              </ul>
              <p class="panel-note">
                {tr("status.latestPolicy", {}, $locale)}
              </p>
            </article>
          </section>

          <section class="dashboard-grid lower">
            <article class="panel project-panel">
              <div class="panel-heading">
                <div>
                  <span class="section-kicker">{tr("projects.coverage", {}, $locale)}</span>
                  <h2>{tr("projects.densest", {}, $locale)}</h2>
                </div>
                <button class="text-button" onclick={() => goTo("projects")}>{tr("nav.projects", {}, $locale)} <ArrowUpRight size={15} /></button>
              </div>
              <div class="project-bars">
                {#each projectRows.slice(0, 7) as project}
                  <button onclick={() => showProjectAssets(project.name)}>
                    <span class="project-name">{project.name}</span>
                    <span class="bar-track">
                      <i style={`width:${Math.max(8, (project.assets / (projectRows[0]?.assets || 1)) * 100)}%`}></i>
                    </span>
                    <strong>{project.assets}</strong>
                  </button>
                {/each}
              </div>
            </article>
            <article class="panel system-panel">
              <div class="panel-heading">
                <div>
                  <span class="section-kicker">{tr("strategy.kicker", {}, $locale)}</span>
                  <h2>{tr("strategy.title", {}, $locale)}</h2>
                </div>
                <Settings2 size={20} />
              </div>
              <ol class="strategy-list">
                <li class="done">
                  <span><Check size={14} /></span>
                  <div><strong>{tr("strategy.watchTitle", {}, $locale)}</strong><small>{tr("strategy.watchBody", {}, $locale)}</small></div>
                </li>
                <li>
                  <span>2</span>
                  <div><strong>{tr("strategy.localTitle", {}, $locale)}</strong><small>{tr("strategy.localBody", {}, $locale)}</small></div>
                </li>
                <li>
                  <span>3</span>
                  <div><strong>{tr("strategy.fullTitle", {}, $locale)}</strong><small>{tr(PUBLIC_RELEASE ? "strategy.fullPublic" : "strategy.fullPrivate", {}, $locale)}</small></div>
                </li>
                <li>
                  <span>4</span>
                  <div><strong>{tr("strategy.candidateTitle", {}, $locale)}</strong><small>{tr("strategy.candidateBody", {}, $locale)}</small></div>
                </li>
              </ol>
            </article>
          </section>
        {:else if currentView === "assets"}
          <AssetExplorer
            bind:projectFilter
            {groups}
            {query}
            {selectedIds}
            {toggleSelection}
            {addSelection}
            {clearSelection}
            {openAsset}
            {governanceById}
            {manageSelection}
            goToExports={() => goTo("exports")}
          />
        {:else if currentView === "projects"}
          <ProjectAtlas
            {groups}
            projects={projectRows}
            {openAsset}
            {showProjectAssets}
          />
        {:else if currentView === "updates"}
          <EvolutionWorkbench
            {groups}
            {refreshing}
            {refreshResult}
            runRefresh={() => runRefresh("full")}
            {openAsset}
          />
        {:else if currentView === "exports"}
          <ExportCenter
            bind:exportFormat
            {exportMode}
            {exporting}
            {analyzingRelations}
            {relationPreview}
            {selectedAgents}
            {selectedSkills}
            plannedAssetCount={plannedAssetCount()}
            exportReady={exportReady()}
            chooseMode={chooseExportMode}
            {analyzeCompanions}
            goToAssets={() => goTo("assets")}
            runExport={() => runExport([...selectedIds])}
            {exportHistory}
            {clearExportHistory}
          />
        {:else if currentView === "controls"}
          <UnifiedControlCenter {groups} {openAsset} />
        {/if}
      </div>
    {/if}
  </main>
</div>

{#if selectedAsset}
  <button class="drawer-backdrop" aria-label={tr("common.close", {}, $locale)} onclick={() => (selectedAsset = null)}></button>
  <aside class="detail-drawer" aria-label={tr("drawer.details", {}, $locale)}>
    <header>
      <div class="asset-glyph large {selectedAsset.kind.toLowerCase()}">
        {#if selectedAsset.kind.toLowerCase() === "agent"}<Bot size={22} />{:else}<Sparkles size={22} />{/if}
      </div>
      <button class="icon-button" aria-label={tr("common.close", {}, $locale)} onclick={() => (selectedAsset = null)}><X size={18} /></button>
    </header>
    <div class="drawer-body">
      <span class="eyebrow">{selectedAsset.kind.toUpperCase()}</span>
      <h1>{selectedAsset.name}</h1>
      <p class="drawer-description">{selectedAsset.description || tr("common.noDescription", {}, $locale)}</p>
      <div class="drawer-status">
        <span class="status-chip {lifecycleTone(selectedAsset)}">{lifecycleLabel(selectedAsset)}</span>
        <span class="license-chip">{governanceById.get(selectedAsset.logical_id)?.license?.decision || selectedAsset.license || tr("status.licensePending", {}, $locale)}</span>
      </div>

      {#if selectedAsset.variants.length > 1}
        <VariantComparison
          asset={selectedAsset}
          record={governanceById.get(selectedAsset.logical_id)}
          onChoose={async (sha) => {
            selectedDefinitionHash = sha;
            candidateRequest += 1;
            await tick();
            const candidate = document.querySelector<HTMLInputElement>("#asset-governance-candidate input:checked");
            candidate?.scrollIntoView({ block: "center" });
            candidate?.focus({ preventScroll: true });
          }}
        />
      {/if}

      <section>
        <h2>{tr("drawer.usage", {}, $locale)}</h2>
        <p>{selectedAsset.usage || tr("drawer.usageFallback", {}, $locale)}</p>
      </section>

      <section>
        <h2>{tr("drawer.toolsProjects", {}, $locale)}</h2>
        <div class="tag-list">
          {#each selectedAsset.tools ?? [] as tool}<span>{tool}</span>{/each}
          {#each selectedAsset.projects ?? [] as project}<span class="project-tag">{project}</span>{/each}
          {#if (selectedAsset.tools?.length ?? 0) + (selectedAsset.projects?.length ?? 0) === 0}
            <span>{tr("drawer.unlinked", {}, $locale)}</span>
          {/if}
        </div>
      </section>

      <section>
        <h2>{tr("drawer.versions", {}, $locale)} <em>{selectedAsset.variants.length}</em></h2>
        <div class="variant-list">
          {#each selectedAsset.variants as variant, index}
            <article>
              <div class="variant-top">
                <span>{tr("drawer.version", { index: index + 1 }, $locale)}</span>
                <code title={variant.sha256}>{compactHash(variant.sha256)}</code>
              </div>
              {#each variant.locations.slice(0, 4) as location}
                <div class="location">
                  <strong>{location.source_class || tr("source.local", {}, $locale)}</strong>
                  <code title={location.path}>{location.path}</code>
                </div>
              {/each}
              {#if variant.locations.length > 4}<small>{tr("drawer.moreLocations", { count: variant.locations.length - 4 }, $locale)}</small>{/if}
            </article>
          {/each}
        </div>
      </section>

      <section>
        <h2>{tr("drawer.verification", {}, $locale)}</h2>
        {#if (selectedAsset.ready_versions?.length ?? 0) > 0}
          {#each selectedAsset.ready_versions ?? [] as version}
            <div class="ready-version">
              <PackageCheck size={17} />
              <span><strong>{version.version || tr("drawer.fixedVersion", {}, $locale)}</strong><small>{version.canonical_path || version.path || version.sha256 || tr("drawer.registered", {}, $locale)}</small></span>
            </div>
          {/each}
        {:else}
          <div class="not-ready">
            <CircleAlert size={17} />
            <span>{tr("drawer.notReady", {}, $locale)}</span>
          </div>
        {/if}
      </section>

      <DefinitionPreview asset={selectedAsset} preferredHash={selectedDefinitionHash} />


      {#key assetVersionKey(selectedAsset)}
      <AssetGovernancePanel
        asset={selectedAsset}
        preferredHash={selectedDefinitionHash}
        preferenceRevision={candidateRequest}
        record={governanceById.get(selectedAsset.logical_id)}
        onUpdated={updateGovernance}
        {reportError}
      />
      {/key}
    </div>
    <footer>
      <label class="checkbox action-check">
        <input
          type="checkbox"
          checked={selectedIds.has(selectedAsset.logical_id)}
          onchange={() => toggleSelection(selectedAsset!.logical_id)}
        />
        <span><Check size={12} /></span>
        {tr("drawer.addExport", {}, $locale)}
      </label>
      {#if selectedAsset.kind.toLowerCase() === "agent"}
        <button class="secondary-button small" onclick={() => openSingleExport(selectedAsset!, "agents_only")}>
          <Download size={15} /> {tr("drawer.agentOnly", {}, $locale)}
        </button>
        <button class="primary-button small" onclick={() => openSingleExport(selectedAsset!, "agent_with_skills")}>
          <Boxes size={15} /> Agent + Skills
        </button>
      {:else}
        <button class="primary-button small" onclick={() => openSingleExport(selectedAsset!, "skills_only")}>
          <Download size={15} /> {tr("drawer.skillOnly", {}, $locale)}
        </button>
      {/if}
    </footer>
  </aside>
{/if}

{#if toast}
  <div class="toast"><Check size={16} /> {toast}</div>
{/if}

{#if error && envelope}
  <div class="error-toast">
    <CircleAlert size={16} /><span>{error}</span><button aria-label={tr("common.close", {}, $locale)} onclick={() => (error = "")}><X size={15} /></button>
  </div>
{/if}
