<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { CheckCircle2, FilePenLine, FlaskConical, FolderOpen, Save, ShieldCheck, Sparkles } from "@lucide/svelte";
  import {
    applyOptimizationDraft,
    confirmAssetLicense,
    confirmAssetVerification,
    createOptimizationDraft,
    runAssetAudit,
    selectAssetVariant
  } from "../api";
  import type { AssetAuditReport, AssetGovernanceRecord, AssetGroup, OptimizationDraftReceipt } from "../types";
  import { locale, tr } from "../i18n";

  export let asset: AssetGroup;
  export let record: AssetGovernanceRecord | undefined;
  export let onUpdated: (record: AssetGovernanceRecord) => void;
  export let reportError: (reason: unknown) => void;

  let candidateSha = record?.selected_sha256 || (asset.variants.length === 1 ? asset.variants[0]?.sha256 ?? "" : "");
  let licenseDecision = record?.license?.decision ?? (asset.license && !asset.license.includes("待") ? asset.license : "");
  let licenseEvidence = record?.license?.evidence ?? "";
  let verificationEvidence = record?.verification?.evidence ?? "";
  let report: AssetAuditReport | null = null;
  let busy = "";
  let previousAssetId = asset.logical_id;

  $: if (asset.logical_id !== previousAssetId) {
    previousAssetId = asset.logical_id;
    candidateSha = record?.selected_sha256 || (asset.variants.length === 1 ? asset.variants[0]?.sha256 ?? "" : "");
    licenseDecision = record?.license?.decision ?? (asset.license && !asset.license.includes("待") ? asset.license : "");
    licenseEvidence = record?.license?.evidence ?? "";
    verificationEvidence = record?.verification?.evidence ?? "";
    report = null;
  }

  function compactHash(value: string) {
    return value ? `${value.slice(0, 10)}…${value.slice(-6)}` : tr("model.notSelected", {}, $locale);
  }

  async function chooseVariant() {
    if (!candidateSha) return;
    if (!window.confirm(`${tr("governance.variant", {}, $locale)}?`)) return;
    busy = "variant";
    try { onUpdated(await selectAssetVariant(asset.logical_id, candidateSha)); }
    catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function confirmLicense() {
    if (!window.confirm(`${tr("governance.license", {}, $locale)}?`)) return;
    busy = "license";
    try { onUpdated(await confirmAssetLicense(asset.logical_id, licenseDecision, licenseEvidence)); }
    catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function deepAudit() {
    busy = "audit";
    try {
      const result = await runAssetAudit([asset.logical_id], true);
      report = result.reports[0] ?? null;
      if (!report) throw new Error(tr("audit.blocked", {}, $locale));
    } catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function verify() {
    if (!candidateSha || !report || report.status === "blocked") return;
    if (!window.confirm(`${tr("governance.verify", {}, $locale)}?`)) return;
    busy = "verify";
    try { onUpdated(await confirmAssetVerification(asset.logical_id, candidateSha, verificationEvidence)); }
    catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function createDraft() {
    if (!candidateSha) return;
    busy = "draft";
    try { onUpdated(await createOptimizationDraft(asset.logical_id, candidateSha)); }
    catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function revealDraft(draft: OptimizationDraftReceipt) {
    try { await revealItemInDir(draft.working_file); }
    catch (reason) { reportError(reason); }
  }

  async function applyDraft(draft: OptimizationDraftReceipt) {
    if (!window.confirm(`${tr("governance.draft", {}, $locale)}?`)) return;
    busy = draft.id;
    try { onUpdated(await applyOptimizationDraft(asset.logical_id, draft.id)); }
    catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }
</script>

<section class="governance-panel">
  <header>
    <div><span class="eyebrow">ACTION WORKFLOW</span><h2>{tr("governance.title", {}, $locale)}</h2></div>
    {#if record?.pending_refresh}<span class="status-chip warn">{tr("assets.optimizedPending", {}, $locale)}</span>
    {:else if record?.verification}<span class="status-chip good">{tr("assets.verified", {}, $locale)}</span>
    {:else if record?.selected_sha256}<span class="status-chip neutral">{tr("assets.versionSelected", {}, $locale)}</span>{/if}
  </header>
  <p class="governance-boundary">{tr("path.manual", {}, $locale)}</p>

  <article class="governance-step">
    <div class="step-number">1</div>
    <div class="step-content">
      <h3>{tr("governance.variant", {}, $locale)} <small>{asset.variants.length}</small></h3>
      <div class="variant-choice-list">
        {#each asset.variants as variant, index}
          <label class:selected={candidateSha === variant.sha256}>
            <input type="radio" name={`variant-${asset.logical_id}`} bind:group={candidateSha} value={variant.sha256} />
            <span><strong>{tr("drawer.version", { index: index + 1 }, $locale)} · {compactHash(variant.sha256)}</strong><small>{variant.locations[0]?.source_class || tr("source.local", {}, $locale)} · {tr("common.items", { count: variant.locations.length }, $locale)}</small></span>
          </label>
        {/each}
      </div>
      <button class="secondary-button small" disabled={!candidateSha || busy === "variant" || record?.selected_sha256 === candidateSha} onclick={chooseVariant}><Save size={14} />{tr(record?.selected_sha256 === candidateSha ? "assets.versionSelected" : "common.confirm", {}, $locale)}</button>
    </div>
  </article>

  <article class="governance-step">
    <div class="step-number">2</div>
    <div class="step-content">
      <h3>{tr("governance.license", {}, $locale)} {#if record?.license}<CheckCircle2 size={15} />{/if}</h3>
      <input bind:value={licenseDecision} placeholder="MIT" />
      <textarea bind:value={licenseEvidence} placeholder={tr("status.truthBoundary", {}, $locale)}></textarea>
      <button class="secondary-button small" disabled={!licenseDecision.trim() || licenseEvidence.trim().length < 4 || busy === "license"} onclick={confirmLicense}><ShieldCheck size={14} />{tr("common.confirm", {}, $locale)}</button>
    </div>
  </article>

  <article class="governance-step">
    <div class="step-number">3</div>
    <div class="step-content">
      <h3>{tr("governance.verify", {}, $locale)} {#if record?.verification}<CheckCircle2 size={15} />{/if}</h3>
      <button class="secondary-button small" disabled={busy === "audit"} onclick={deepAudit}><FlaskConical size={14} />{tr(busy === "audit" ? "common.loading" : "audit.deep", {}, $locale)}</button>
      {#if report}
        <div class="inline-audit-result {report.status}"><strong>{report.score} · {tr(report.status === "blocked" ? "audit.blocked" : report.status === "usable" ? "audit.usable" : "audit.optimize", {}, $locale)}</strong><span>{report.checks.filter((check) => check.status === "pass").length}/{report.checks.length}</span></div>
        <textarea bind:value={verificationEvidence} placeholder={tr("status.truthBoundary", {}, $locale)}></textarea>
        <button class="primary-button small" disabled={!candidateSha || report.status === "blocked" || verificationEvidence.trim().length < 8 || busy === "verify"} onclick={verify}><ShieldCheck size={14} />{tr("assets.verified", {}, $locale)}</button>
      {/if}
    </div>
  </article>

  <article class="governance-step">
    <div class="step-number">4</div>
    <div class="step-content">
      <h3>{tr("governance.draft", {}, $locale)}</h3>
      <p>{tr("path.manual", {}, $locale)}</p>
      <button class="secondary-button small" disabled={!candidateSha || busy === "draft"} onclick={createDraft}><Sparkles size={14} />{tr("common.add", {}, $locale)}</button>
      {#each (record?.drafts ?? []).slice(0, 3) as draft}
        <div class="draft-receipt">
          <div><strong>{draft.id}</strong><small>{tr(draft.status === "applied" ? "common.apply" : "common.edit", {}, $locale)} · {compactHash(draft.status === "applied" ? draft.output_sha256 : draft.source_sha256)}</small></div>
          <button class="text-button" onclick={() => revealDraft(draft)}><FolderOpen size={13} />{tr("common.details", {}, $locale)}</button>
          {#if draft.status === "draft"}<button class="danger-outline" disabled={busy === draft.id} onclick={() => applyDraft(draft)}><FilePenLine size={13} />{tr("common.apply", {}, $locale)}</button>{/if}
        </div>
      {/each}
      {#if record?.pending_refresh}<div class="governance-warning">{tr("assets.optimizedPending", {}, $locale)}</div>{/if}
    </div>
  </article>
</section>
