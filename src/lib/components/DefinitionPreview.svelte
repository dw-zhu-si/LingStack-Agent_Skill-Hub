<script lang="ts">
  import type { AssetDefinition, AssetGroup } from "../types";
  import { readAssetDefinition, supportsDefinitionReads } from "../api";
  import { locale, tr } from "../i18n";

  export let asset: AssetGroup;
  export let preferredHash = "";
  let selectedHash = "";
  let assetKey = "";
  let definition: AssetDefinition | null = null;
  let loading = false;
  let error = "";
  let find = "";
  let request = 0;
  const desktop = supportsDefinitionReads();
  $: currentAssetKey = JSON.stringify([asset.logical_id, asset.variants, preferredHash]);
  $: if (assetKey !== currentAssetKey) {
    assetKey = currentAssetKey;
    selectedHash = asset.variants.some(variant => variant.sha256 === preferredHash)
      ? preferredHash
      : asset.variants[0]?.sha256 ?? "";
    reset();
  }
  $: lines = definition?.content.split("\n") ?? [];
  $: needle = find.trim().toLocaleLowerCase();
  $: matchingLines = needle ? lines.filter(line => line.toLocaleLowerCase().includes(needle)) : [];
  $: matchCount = needle ? lines.reduce((count, line) => count + line.toLocaleLowerCase().split(needle).length - 1, 0) : 0;

  function reset() {
    request += 1;
    loading = false;
    definition = null;
    error = "";
    find = "";
  }
  async function load() {
    const id = ++request;
    error = "";
    definition = null;
    loading = true;
    try {
      const result = await readAssetDefinition(asset.logical_id, selectedHash);
      if (id === request) definition = result;
    } catch (cause) {
      if (id === request) error = String(cause);
    } finally {
      if (id === request) loading = false;
    }
  }
</script>

<section class="definition-preview" aria-busy={loading}>
  <h3>{tr("definition.title", {}, $locale)}</h3>
  {#if !desktop}
    <p role="status">{tr("definition.desktopRequired", {}, $locale)}</p>
  {:else}
    <div class="definition-controls">
      <select bind:value={selectedHash} onchange={reset} aria-label={tr("common.versions", {}, $locale)}>
        {#each asset.variants as variant}
          <option value={variant.sha256}>{variant.sha256.slice(0, 12)} · {variant.locations[0]?.path ?? ""}</option>
        {/each}
      </select>
      <button class="secondary-button small" onclick={load} disabled={loading || !selectedHash}>
        {tr(loading ? "definition.loading" : "definition.load", {}, $locale)}
      </button>
    </div>
    {#if error}<p class="definition-error" role="alert">{error}</p>{/if}
    {#if definition}
      <p class="definition-path" dir="auto">{definition.path}</p>
      <p>{tr("definition.stats", { lines: definition.line_count, bytes: definition.byte_count, tokens: definition.estimated_tokens }, $locale)}</p>
      <p class="definition-help">{tr("definition.estimate", {}, $locale)}</p>
      <label class="definition-find">
        {tr("definition.find", {}, $locale)}
        <input type="search" bind:value={find} />
      </label>
      {#if needle}<p role="status">{tr("definition.matches", { count: matchCount }, $locale)}</p>{/if}
      {#if definition.content}
        {#if needle && matchingLines.length > 200}<p role="status">{tr("explorer.truncated", {}, $locale)}</p>{/if}
        <textarea class="definition-content" readonly rows="16" aria-label={tr("definition.title", {}, $locale)} dir="auto" value={needle ? matchingLines.slice(0, 200).join("\n") : definition.content}></textarea>
      {:else}<p>{tr("definition.empty", {}, $locale)}</p>{/if}
    {/if}
  {/if}
</section>

<style>
  .definition-preview { margin-block: 24px; min-width: 0; }
  .definition-controls { display: flex; flex-wrap: wrap; gap: 8px; }
  select { min-width: 0; max-width: 100%; flex: 1 1 200px; }
  .definition-path { overflow-wrap: anywhere; font-size: 12px; }
  .definition-help { font-size: 12px; line-height: 1.6; opacity: .8; }
  .definition-find { display: grid; gap: 8px; font-size: 12px; }
  input { min-width: 0; width: 100%; }
  .definition-content { width: 100%; resize: vertical; font-family: ui-monospace, monospace; font-size: 12px; line-height: 1.65; max-height: 420px; overflow: auto; padding: 12px; border: 1px solid var(--line); border-radius: 8px; }
  .definition-error { color: var(--red); overflow-wrap: anywhere; }
</style>
