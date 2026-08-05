<script lang="ts">
  import { ArrowUpRight, Bot, Boxes, FolderKanban, Search, Sparkles, Wrench } from "@lucide/svelte";
  import type { AssetGroup, ProjectSummary } from "../types";
  import { locale, tr } from "../i18n";

  export let groups: AssetGroup[];
  export let projects: ProjectSummary[];
  export let openAsset: (asset: AssetGroup) => void;
  export let showProjectAssets: (project: string) => void;

  let projectQuery = "";
  let projectSort = "assets";
  let selectedName = projects[0]?.name ?? "";

  $: normalizedQuery = projectQuery.trim().toLocaleLowerCase();
  $: visibleProjects = projects
    .filter((project) => !normalizedQuery || project.name.toLocaleLowerCase().includes(normalizedQuery))
    .sort((left, right) => {
      if (projectSort === "ready") return coverage(right) - coverage(left) || right.assets - left.assets;
      if (projectSort === "name") return left.name.localeCompare(right.name, $locale);
      return right.assets - left.assets || left.name.localeCompare(right.name, $locale);
    });
  $: if (!visibleProjects.some((project) => project.name === selectedName)) {
    selectedName = visibleProjects[0]?.name ?? "";
  }
  $: selectedProject = projects.find((project) => project.name === selectedName) ?? null;
  $: selectedAssets = selectedProject
    ? groups.filter((asset) => (asset.projects ?? []).includes(selectedProject!.name))
    : [];
  $: attentionCount = selectedAssets.filter(
    (asset) => asset.variants.length > 1 || (asset.ready_versions?.length ?? 0) === 0
  ).length;
  $: topTools = aggregateTools(selectedAssets).slice(0, 6);

  function coverage(project: ProjectSummary): number {
    return project.assets ? Math.round((project.ready / project.assets) * 100) : 0;
  }

  function aggregateTools(assets: AssetGroup[]): Array<[string, number]> {
    const counts = new Map<string, number>();
    for (const asset of assets) {
      for (const tool of asset.tools ?? []) counts.set(tool, (counts.get(tool) ?? 0) + 1);
    }
    return [...counts.entries()].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0], $locale));
  }
</script>

<section class="compact-heading atlas-heading">
  <div>
    <span class="eyebrow">PROJECT CAPABILITY ATLAS</span>
    <h1>{tr("projects.title", {}, $locale)}</h1>
    <p>{tr("projects.subtitle", {}, $locale)}</p>
  </div>
  <span class="atlas-total">{tr("projects.registered", { count: projects.length }, $locale)}</span>
</section>

<section class="atlas-toolbar" aria-label={tr("common.stateFilter", {}, $locale)}>
  <label class="atlas-search">
    <Search size={15} aria-hidden="true" />
    <span class="sr-only">{tr("common.search", {}, $locale)}</span>
    <input bind:value={projectQuery} placeholder={tr("projects.search", {}, $locale)} />
  </label>
  <label>
    <span>{tr("common.sort", {}, $locale)}</span>
    <select bind:value={projectSort}>
      <option value="assets">{tr("common.assets", {}, $locale)}</option>
      <option value="ready">{tr("projects.coverageLabel", {}, $locale)}</option>
      <option value="name">{tr("nav.projects", {}, $locale)}</option>
    </select>
  </label>
</section>

<section class="project-atlas">
  <aside class="atlas-index panel" aria-label={tr("nav.projects", {}, $locale)}>
    <div class="atlas-list-head"><span>{tr("nav.projects", {}, $locale)}</span><span>{tr("projects.capabilityMix", {}, $locale)}</span><span>{tr("projects.coverageLabel", {}, $locale)}</span></div>
    <div class="atlas-list">
      {#each visibleProjects as project (project.name)}
        <button class:active={project.name === selectedName} aria-pressed={project.name === selectedName} onclick={() => (selectedName = project.name)}>
          <span class="project-marker"><FolderKanban size={15} /></span>
          <span class="atlas-project-name"><strong>{project.name}</strong><small>{tr("common.items", { count: project.assets }, $locale)}</small></span>
          <span class="capability-mix" title={`${project.agents} Agent / ${project.skills} Skill`}>
            <i class="agents" style={`width:${project.assets ? (project.agents / project.assets) * 100 : 0}%`}></i>
            <i class="skills" style={`width:${project.assets ? (project.skills / project.assets) * 100 : 0}%`}></i>
          </span>
          <strong class="coverage-value">{coverage(project)}%</strong>
        </button>
      {/each}
      {#if visibleProjects.length === 0}<div class="atlas-empty">{tr("common.none", {}, $locale)}</div>{/if}
    </div>
  </aside>

  <article class="atlas-detail panel">
    {#if selectedProject}
      <header>
        <div class="project-icon large"><FolderKanban size={22} /></div>
        <div><span class="section-kicker">SELECTED PROJECT</span><h2>{selectedProject.name}</h2></div>
        <button class="secondary-button small" onclick={() => showProjectAssets(selectedProject!.name)}>{tr("projects.viewAssets", {}, $locale)} <ArrowUpRight size={14} /></button>
      </header>
      <div class="atlas-metrics">
        <div><Boxes size={15} /><span><strong>{selectedProject.assets}</strong><small>{tr("common.assets", {}, $locale)}</small></span></div>
        <div><Bot size={15} /><span><strong>{selectedProject.agents}</strong><small>Agent</small></span></div>
        <div><Sparkles size={15} /><span><strong>{selectedProject.skills}</strong><small>Skill</small></span></div>
        <div><Wrench size={15} /><span><strong>{attentionCount}</strong><small>{tr("projects.attention", {}, $locale)}</small></span></div>
      </div>
      <section class="atlas-readiness">
        <div><strong>{tr("projects.coverageLabel", {}, $locale)}</strong><span>{selectedProject.ready} / {selectedProject.assets} · {coverage(selectedProject)}%</span></div>
        <div class="atlas-progress"><i style={`width:${coverage(selectedProject)}%`}></i></div>
      </section>
      <section class="atlas-tools">
        <h3>{tr("projects.primaryTools", {}, $locale)}</h3>
        <div>{#each topTools as [tool, count]}<span>{tool}<em>{count}</em></span>{/each}{#if topTools.length === 0}<small>{tr("common.none", {}, $locale)}</small>{/if}</div>
      </section>
      <section class="atlas-assets">
        <div class="subsection-heading"><h3>{tr("projects.representative", {}, $locale)}</h3><small>{tr("projects.attention", {}, $locale)}</small></div>
        {#each [...selectedAssets].sort((a, b) => b.variants.length - a.variants.length).slice(0, 7) as asset (asset.logical_id)}
          <button onclick={() => openAsset(asset)}>
            <span class="asset-glyph {asset.kind.toLowerCase()}">{#if asset.kind.toLowerCase() === "agent"}<Bot size={14} />{:else}<Sparkles size={14} />{/if}</span>
            <span><strong>{asset.name}</strong><small>{tr("drawer.versions", {}, $locale)} · {asset.variants.length}</small></span>
            <em>{(asset.ready_versions?.length ?? 0) > 0 ? tr("assets.ready", {}, $locale) : tr("assets.pendingVerification", {}, $locale)}</em>
          </button>
        {/each}
      </section>
    {:else}
      <div class="atlas-detail-empty"><FolderKanban size={28} /><span>{tr("nav.projects", {}, $locale)}</span></div>
    {/if}
  </article>
</section>
