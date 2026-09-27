<script lang="ts">
  import VersionDecisionPanel from "./VersionDecisionPanel.svelte";
  import { hasValidVerification } from "../assetState";
  import type { AssetDefinition, AssetGovernanceRecord, AssetGroup } from "../types";
  import { readAssetDefinition, supportsDefinitionReads } from "../api";
  import { locale, tr } from "../i18n";
  import { compareDefinitions, extractComparableDefinition } from "../variantComparison";

  export let asset: AssetGroup;
  export let record: AssetGovernanceRecord | undefined;
  export let onChoose: (sha: string) => void;

  const desktop = supportsDefinitionReads();
  const DISPLAY_LIMIT = 300;
  const LINE_LIMIT = 2000;
  let leftHash = "";
  let rightHash = "";
  let previousKey = "";
  let request = 0;
  let loading = false;
  let onlyChanges = true;
  let page = 1;
  let compareMode = "core";
  let coreAvailable = false;
  let leftDefinition: AssetDefinition | null = null;
  let rightDefinition: AssetDefinition | null = null;
  let leftError = "";
  let rightError = "";
  let comparisonError = "";
  let result: ReturnType<typeof compareDefinitions> | null = null;

  $: currentKey = JSON.stringify([asset.logical_id, asset.variants]);
  $: if (currentKey !== previousKey) {
    previousKey = currentKey;
    leftHash = asset.variants.some(v => v.sha256 === record?.selected_sha256)
      ? record!.selected_sha256 : asset.variants[0]?.sha256 ?? "";
    rightHash = asset.variants.find(v => v.sha256 !== leftHash)?.sha256 ?? "";
    invalidate();
  }
  $: sides = [
    { name: tr("comparison.left", {}, $locale), hash: leftHash, definition: leftDefinition, error: leftError },
    { name: tr("comparison.right", {}, $locale), hash: rightHash, definition: rightDefinition, error: rightError }
  ];
  $: filteredRows = result?.rows.filter(row => !onlyChanges || row.kind !== "equal") ?? [];
  $: totalPages = Math.max(1, Math.ceil(filteredRows.length / DISPLAY_LIMIT));
  $: currentPage = Math.min(page, totalPages);
  $: visibleRows = filteredRows.slice((currentPage - 1) * DISPLAY_LIMIT, currentPage * DISPLAY_LIMIT);
  $: clippedLines = visibleRows.some(row => row.text.length > LINE_LIMIT);
  $: validEvidence = hasValidVerification(asset, record)
    && [leftHash, rightHash].includes(record?.verification?.sha256 ?? "");

  function lineEnding(text: string): string {
    return text.endsWith("\r\n") ? "CRLF" : text.endsWith("\n") ? "LF" : "EOF";
  }

  function lineText(text: string): string {
    return text.replace(/\r?\n$/, "");
  }

  function invalidate() {
    request += 1;
    loading = false;
    leftDefinition = null;
    rightDefinition = null;
    leftError = "";
    rightError = "";
    comparisonError = "";
    result = null;
    coreAvailable = false;
    page = 1;
  }

  function computeComparison() {
    if (!leftDefinition || !rightDefinition) return;
    page = 1;
    const left = compareMode === "core" ? extractComparableDefinition(leftDefinition.content, leftDefinition.path).text : leftDefinition.content;
    const right = compareMode === "core" ? extractComparableDefinition(rightDefinition.content, rightDefinition.path).text : rightDefinition.content;
    result = compareDefinitions(left, right);
  }

  async function compare() {
    if (!desktop || !leftHash || !rightHash || leftHash === rightHash || loading) return;
    invalidate();
    const sequence = request;
    const logicalId = asset.logical_id;
    loading = true;
    const [left, right] = await Promise.allSettled([
      readAssetDefinition(logicalId, leftHash), readAssetDefinition(logicalId, rightHash)
    ]);
    if (sequence !== request) return;
    if (left.status === "fulfilled") leftDefinition = left.value;
    else leftError = String(left.reason);
    if (right.status === "fulfilled") rightDefinition = right.value;
    else rightError = String(right.reason);
    try {
      if (leftDefinition && rightDefinition) {
        coreAvailable = extractComparableDefinition(leftDefinition.content, leftDefinition.path).extracted
          && extractComparableDefinition(rightDefinition.content, rightDefinition.path).extracted;
        compareMode = coreAvailable ? "core" : "raw";
        computeComparison();
      }
    } catch (cause) {
      comparisonError = String(cause);
    } finally {
      loading = false;
    }
  }
</script>

<section class="variant-comparison" aria-busy={loading}>
  <h3>{tr("comparison.title", {}, $locale)}</h3>
  <p class="hint">{tr("comparison.hint", {}, $locale)}</p>
  <div class="selectors">
    <label>{tr("comparison.left", {}, $locale)}<select bind:value={leftHash} onchange={invalidate}>
      {#each asset.variants as variant}<option value={variant.sha256}>{variant.locations[0]?.source_class || tr("source.local", {}, $locale)} · {variant.sha256.slice(0, 12)}</option>{/each}
    </select></label>
    <label>{tr("comparison.right", {}, $locale)}<select bind:value={rightHash} onchange={invalidate}>
      <option value="">{tr("comparison.select", {}, $locale)}</option>
      {#each asset.variants as variant}<option value={variant.sha256}>{variant.locations[0]?.source_class || tr("source.local", {}, $locale)} · {variant.sha256.slice(0, 12)}</option>{/each}
    </select></label>
  </div>
  {#if !desktop}<p role="status">{tr("comparison.desktop", {}, $locale)}</p>{/if}
  {#if leftHash && leftHash === rightHash}<p role="status">{tr("comparison.different", {}, $locale)}</p>{/if}
  <button class="secondary-button small" onclick={compare} disabled={!desktop || loading || !leftHash || !rightHash || leftHash === rightHash}>{tr(loading ? "comparison.loading" : "comparison.load", {}, $locale)}</button>

  <VersionDecisionPanel {asset} {record} left={leftDefinition} right={rightDefinition} {onChoose} />

  <div class="versions">
    {#each sides as side}
      <article>
        <h4>{side.name}</h4>
        {#if side.hash}
          <p>{tr("explorer.source", {}, $locale)}: {[...new Set(asset.variants.find(v => v.sha256 === side.hash)?.locations.map(l => l.source_class || tr("source.local", {}, $locale)) ?? [])].join(" · ")}</p>
          <code class="hash">SHA-256: {side.hash}</code>
          <p>{tr(record?.selected_sha256 === side.hash ? "comparison.selected" : "comparison.notSelected", {}, $locale)}</p>
          {#if hasValidVerification(asset, record) && record?.verification?.sha256 === side.hash}
            <p>{tr("comparison.verified", {}, $locale)}</p>
            <dl><dt>{tr("comparison.verifiedAt", {}, $locale)}</dt><dd>{record.verification.verified_at}</dd><dt>{tr("comparison.level", {}, $locale)}</dt><dd>{record.verification.truth_level}</dd><dt>{tr("comparison.evidence", {}, $locale)}</dt><dd class="evidence">{record.verification.evidence}</dd></dl>
          {:else}<p>{tr("comparison.noEvidence", {}, $locale)}</p>{/if}
          <details><summary>{tr("comparison.paths", { count: asset.variants.find(v => v.sha256 === side.hash)?.locations.length ?? 0 }, $locale)}</summary>
            <ul>{#each asset.variants.find(v => v.sha256 === side.hash)?.locations ?? [] as location}<li dir="auto">{location.path}</li>{/each}</ul>
          </details>
          {#if side.definition}<p>{tr("comparison.readPath", {}, $locale)}<span dir="auto">{side.definition.path}</span></p><p>{tr("comparison.stats", { lines: side.definition.line_count, bytes: side.definition.byte_count }, $locale)}</p>{/if}
          {#if side.error}<p class="error" role="alert">{tr("comparison.readError", { side: side.name, error: side.error }, $locale)}</p>{/if}
          <button class="secondary-button small" onclick={() => onChoose(side.hash)}>{tr("comparison.choose", { side: side.name }, $locale)}</button>
        {:else}<p>{tr("comparison.noSelection", {}, $locale)}</p>{/if}
      </article>
    {/each}
  </div>
  <p class="hint">{tr(validEvidence ? "comparison.evidenceHint" : "comparison.missingHint", {}, $locale)} {tr("comparison.candidateHint", {}, $locale)}</p>
  {#if comparisonError}<p class="error" role="alert">{tr("comparison.error", { error: comparisonError }, $locale)}</p>{/if}
  {#if result}
    <select bind:value={compareMode} onchange={computeComparison} aria-label={tr("comparison.title", {}, $locale)}>
      {#if coreAvailable}<option value="core">{tr("comparison.core", {}, $locale)}</option>{/if}
      <option value="raw">{tr("comparison.raw", {}, $locale)}</option>
    </select>
    <p class="hint">{tr(compareMode === "core" ? "comparison.coreHint" : "comparison.rawHint", {}, $locale)}</p>
    {#if compareMode === "core" && !result.truncated && result.added === 0 && result.removed === 0}<p class="core-same" role="status">{tr("comparison.coreSame", {}, $locale)}</p>{/if}
    <p role="status">{tr("comparison.summary", { added: result.added, removed: result.removed }, $locale)} {#if result.truncated}{tr("comparison.truncated", {}, $locale)}{/if}</p>
    <label class="changes"><input type="checkbox" bind:checked={onlyChanges} onchange={() => { page = 1; }} />{tr("comparison.changes", {}, $locale)}</label>
    {#if filteredRows.length > DISPLAY_LIMIT || clippedLines}<p role="status">{tr("comparison.displayLimit", { lines: DISPLAY_LIMIT, chars: LINE_LIMIT }, $locale)}</p>{/if}
    {#if visibleRows.length === 0}<p>{tr(result.truncated ? "comparison.emptyPartial" : "comparison.empty", {}, $locale)}</p>
    {:else}
      {#if totalPages > 1}
        <nav class="diff-pagination" aria-label={tr("common.page", { current: currentPage, total: totalPages }, $locale)}>
          <button class="secondary-button small" disabled={currentPage === 1} onclick={() => { page = currentPage - 1; }}>{tr("common.previous", {}, $locale)}</button>
          <span>{tr("common.page", { current: currentPage, total: totalPages }, $locale)}</span>
          <button class="secondary-button small" disabled={currentPage === totalPages} onclick={() => { page = currentPage + 1; }}>{tr("common.next", {}, $locale)}</button>
        </nav>
      {/if}
      {#key currentPage}
      <div class="diff">
        <table><caption>{tr("comparison.caption", {}, $locale)}</caption>
          <thead><tr><th scope="col">{tr("comparison.leftLine", {}, $locale)}</th><th scope="col">{tr("comparison.rightLine", {}, $locale)}</th><th scope="col">{tr("comparison.change", {}, $locale)}</th><th scope="col">{tr("comparison.text", {}, $locale)}</th></tr></thead>
          <tbody>{#each visibleRows as row}<tr class:added={row.kind === "added"} class:removed={row.kind === "removed"}><td>{row.leftLine ?? "—"}</td><td>{row.rightLine ?? "—"}</td><td>{row.kind === "added" ? "+" : row.kind === "removed" ? "−" : "="}</td><td class="text">{lineText(row.text).slice(0, LINE_LIMIT)}{lineText(row.text).length > LINE_LIMIT ? "…" : ""}<span class="line-ending" dir="ltr">{lineEnding(row.text)}</span></td></tr>{/each}</tbody>
        </table>
      </div>
      {/key}
    {/if}
  {/if}
</section>

<style>
  .variant-comparison { margin-block: 24px; min-width: 0; font-size: 12px; line-height: 1.6; }
  h3 { font-size: 16px; } h4 { margin-block: 0 8px; font-size: 13px; }
  .core-same { padding: 12px; background: var(--mint); border-radius: 8px; }
  .hint { color: var(--muted); }
  .selectors, .versions { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; margin-block: 12px; }
  .selectors label { display: grid; gap: 8px; }
  select { width: 100%; min-width: 0; }
  article { padding: 12px; border: 1px solid var(--line); border-radius: 8px; min-width: 0; }
  article p, .hash, dd, li { overflow-wrap: anywhere; }
  .hash { display: block; font-size: 11px; }
  ul { padding-inline-start: 20px; } dd { margin-inline-start: 0; } dt { color: var(--muted); }
  .evidence { white-space: pre-wrap; }
  .error { color: var(--red); overflow-wrap: anywhere; }
  summary { cursor: pointer; }
  .changes { display: flex; gap: 8px; align-items: center; margin-block: 12px; }
  .diff-pagination { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-block: 12px; }
  .diff { max-height: 440px; overflow: auto; border: 1px solid var(--line); border-radius: 8px; }
  table { width: 100%; table-layout: fixed; border-collapse: collapse; font-family: ui-monospace, monospace; font-size: 11px; }
  caption { text-align: start; padding: 8px; font-family: inherit; }
  th, td { padding: 4px; text-align: start; vertical-align: top; border-bottom: 1px solid var(--line); }
  th:nth-child(-n+3) { width: 40px; }
  .line-ending { display: inline-block; margin-inline-start: 8px; padding-inline: 4px; border: 1px solid currentColor; border-radius: 3px; font-size: 9px; opacity: .7; white-space: nowrap; }
  .text { white-space: pre-wrap; overflow-wrap: anywhere; }
  .added { background: var(--mint); color: var(--ink); }
  .removed { background: #fff0ec; color: #72291f; }
  @media (max-width: 620px) { .selectors, .versions { grid-template-columns: minmax(0, 1fr); } }
</style>
