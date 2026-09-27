<script lang="ts">
  import type { AssetDefinition, AssetGovernanceRecord, AssetGroup } from "../types";
  import { evaluateVariantDecision } from "../variantDecision";
  import { locale, tr } from "../i18n";

  export let asset: AssetGroup;
  export let record: AssetGovernanceRecord | undefined;
  export let left: AssetDefinition | null;
  export let right: AssetDefinition | null;
  export let onChoose: (sha: string) => void;

  let targetSource = "";
  let preference: "stable" | "evidence" = "stable";
  let previousId = "";
  $: sources = [...new Set(asset.variants.flatMap(v => v.locations.map(l => l.source_class).filter((s): s is string => !!s)))].sort();
  $: if (previousId !== asset.logical_id) {
    previousId = asset.logical_id;
    targetSource = "";
    preference = "stable";
  }
  $: if (targetSource && !sources.includes(targetSource)) targetSource = "";
  $: decision = evaluateVariantDecision({ asset, record, left, right, targetSource, preference });
  $: recommendedSide = decision.recommendation === left?.sha256 ? "comparison.left" : "comparison.right";
  $: evidenceSides = [
    { label: "comparison.left", state: decision.left },
    { label: "comparison.right", state: decision.right }
  ];
</script>

<section class="decision-panel" aria-label={tr("decision.title", {}, $locale)}>
  <h4>{tr("decision.title", {}, $locale)}</h4>
  <div class="decision-controls">
    <label>{tr("decision.target", {}, $locale)}
      <select bind:value={targetSource}>
        <option value="">{tr("decision.any", {}, $locale)}</option>
        {#each sources as source}<option value={source}>{source}</option>{/each}
      </select>
    </label>
    <label>{tr("decision.preference", {}, $locale)}
      <select bind:value={preference}>
        <option value="stable">{tr("decision.stable", {}, $locale)}</option>
        <option value="evidence">{tr("decision.evidence", {}, $locale)}</option>
      </select>
    </label>
  </div>
  <div class="decision-result" aria-live="polite" aria-atomic="true">
    <strong>{decision.recommendation ? tr("decision.recommend", { side: tr(recommendedSide, {}, $locale) }, $locale) : tr("decision.none", {}, $locale)}</strong>
    <p>{tr(`decision.reason.${decision.reason}`, {}, $locale)}</p>
    {#if left && right && decision.reason !== "unread"}
      <p>{tr(`decision.relation.${decision.relation}`, {}, $locale)}</p>
      {#if decision.truncated}<p>{tr("comparison.truncated", {}, $locale)}</p>{/if}
    {/if}
  </div>
  {#if left && right && decision.reason !== "unread"}
    <div class="decision-evidence">
      {#each evidenceSides as side}
        <div>
          <strong>{tr(side.label, {}, $locale)}</strong>
          {#if targetSource}<span>{tr(side.state.sourceMatch ? "decision.sourceMatch" : "decision.sourceOther", {}, $locale)}</span>{/if}
          <span>{tr(side.state.current ? "comparison.selected" : "comparison.notSelected", {}, $locale)}</span>
          <span>{tr(side.state.verified ? "assets.verified" : "comparison.noEvidence", {}, $locale)}</span>
        </div>
      {/each}
    </div>
  {/if}
  <p>{tr("decision.next", {}, $locale)}</p>
  <button class="primary-button small" disabled={!decision.recommendation} onclick={() => { if (decision.recommendation) onChoose(decision.recommendation); }}>{tr("decision.apply", {}, $locale)}</button>
  <p class="boundary">{tr("decision.scope", {}, $locale)}</p>
</section>

<style>
  .decision-panel { padding: 16px; background: var(--mint); border: 1px solid var(--line); border-radius: 12px; margin-block: 16px; }
  h4 { margin: 0 0 12px; font-size: 16px; }
  .decision-controls, .decision-evidence { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  label { display: grid; gap: 8px; }
  select { width: 100%; min-width: 0; }
  .decision-result { padding-block: 16px 8px; }
  .decision-result strong { font-size: 14px; }
  .decision-evidence > div { display: grid; gap: 6px; padding: 10px; border: 1px solid var(--line); border-radius: 8px; overflow-wrap: anywhere; }
  p { line-height: 1.7; }
  .boundary { color: var(--muted); font-size: 11px; margin-bottom: 0; }
  @media (max-width: 620px) { .decision-controls, .decision-evidence { grid-template-columns: minmax(0, 1fr); } }
</style>
