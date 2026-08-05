<script lang="ts">
  import { FolderCog, FolderOpen, Plus, RotateCcw, Trash2 } from "@lucide/svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { applyToolBinding, deleteCustomBinding, restoreToolBinding, saveCustomBinding } from "../api";
  import type { ControlCenterState, CustomToolBinding, ToolBindingPreview } from "../types";
  import { locale, tr } from "../i18n";

  let { controlState, reload, reportError }: {
    controlState: ControlCenterState;
    reload: () => Promise<void>;
    reportError: (reason: unknown) => void;
  } = $props();
  let showCandidates = $state(false);
  let adding = $state(false);
  let busy = $state("");
  let custom = $state<CustomToolBinding>({ id: "", tool: "", kind: "Skill", source_path: "" });
  let visibleBindings = $derived(controlState.bindings.filter((item) => showCandidates || item.detected || item.custom || item.status === "unified"));

  async function apply(binding: ToolBindingPreview) {
    const migration = binding.requires_migration;
    const warning = `${binding.tool} ${binding.kind}\n${tr("path.manual", {}, $locale)}`;
    if (!window.confirm(warning)) return;
    busy = binding.id;
    try { await applyToolBinding(binding.id, migration); await reload(); }
    catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function restore(receiptId: string) {
    if (!window.confirm(`${tr("common.restore", {}, $locale)}?`)) return;
    busy = receiptId;
    try { await restoreToolBinding(receiptId); await reload(); }
    catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function saveCustom() {
    custom.id = `custom-${Date.now()}`;
    busy = custom.id;
    try {
      await saveCustomBinding(custom);
      custom = { id: "", tool: "", kind: "Skill", source_path: "" };
      adding = false;
      await reload();
    } catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function chooseFolder() {
    try {
      const selected = await open({ directory: true, multiple: false, title: tr("path.chooseFolder", {}, $locale) });
      if (typeof selected === "string") custom.source_path = selected;
    } catch (reason) { reportError(reason); }
  }

  async function removeCustom(binding: ToolBindingPreview) {
    if (!window.confirm(`${tr("common.delete", {}, $locale)}: ${binding.tool}?`)) return;
    try { await deleteCustomBinding(binding.id); await reload(); }
    catch (reason) { reportError(reason); }
  }
</script>

<section class="panel path-panel">
  <header class="control-panel-heading path-heading">
    <div><span class="section-kicker">TOOL PATH ROUTER</span><h2>{tr("path.title", {}, $locale)}</h2><small>{controlState.unified_root}</small></div>
    <div><label class="inline-check"><input type="checkbox" bind:checked={showCandidates} /><span>{tr("common.all", {}, $locale)}</span></label><button class="secondary-button small" onclick={() => (adding = true)}><Plus size={15} />{tr("common.add", {}, $locale)}</button></div>
  </header>
  <div class="manual-banner"><FolderCog size={18} /><span>{tr(controlState.store_sandbox ? "path.storeSandbox" : "path.manual", {}, $locale)}</span></div>
  <div class="binding-table">
    <div class="binding-head"><span>{tr("common.typeFilter", {}, $locale)}</span><span>{tr("controls.paths", {}, $locale)}</span><span>{tr("common.status", {}, $locale)}</span><span>{tr("common.apply", {}, $locale)}</span></div>
    {#each visibleBindings as binding}
      <article>
        <div class="binding-tool"><strong>{binding.tool}</strong><small>{binding.kind}{binding.custom ? ` · ${tr("common.edit", {}, $locale)}` : ""}</small></div>
        <code title={binding.source_path}>{binding.source_path}</code>
        <div class="binding-message"><span class="binding-state {binding.status}">{tr(binding.status === "unified" ? "common.enabled" : binding.detected ? "common.details" : "common.none", {}, $locale)}</span><small>{binding.message}</small></div>
        <div class="binding-actions">
          {#if binding.can_apply}<button disabled={busy === binding.id} onclick={() => apply(binding)}>{tr("path.switch", {}, $locale)}</button>{/if}
          {#if binding.custom}<button aria-label={tr("common.delete", {}, $locale)} class="danger-text" onclick={() => removeCustom(binding)}><Trash2 size={14} /></button>{/if}
        </div>
      </article>
    {/each}
  </div>
</section>

{#if controlState.receipts.length > 0}
  <section class="panel path-receipts">
    <div class="control-panel-heading"><div><span class="section-kicker">ROLLBACK RECEIPTS</span><h2>{tr("controls.paths", {}, $locale)}</h2></div></div>
    {#each controlState.receipts.slice(0, 8) as receipt}
      <article><div><strong>{receipt.binding_id}</strong><small>{receipt.applied_at}</small></div><code>{receipt.backup_path || tr("common.none", {}, $locale)}</code>{#if receipt.status === "applied"}<button disabled={busy === receipt.id} onclick={() => restore(receipt.id)}><RotateCcw size={14} />{tr("common.restore", {}, $locale)}</button>{/if}</article>
    {/each}
  </section>
{/if}

{#if adding}
  <div class="control-modal-backdrop" role="presentation">
    <form class="control-modal" autocomplete="off" onsubmit={(event) => { event.preventDefault(); saveCustom(); }}>
      <header><div><span class="section-kicker">CUSTOM TOOL</span><h2>{tr("common.add", {}, $locale)}</h2></div><button type="button" onclick={() => (adding = false)}>{tr("common.cancel", {}, $locale)}</button></header>
      <p>{tr(controlState.store_sandbox ? "path.storeSandbox" : "path.manual", {}, $locale)}</p>
      <label><span>{tr("model.displayName", {}, $locale)}</span><input bind:value={custom.tool} required autocomplete="off" /></label>
      <label><span>{tr("common.typeFilter", {}, $locale)}</span><select bind:value={custom.kind}><option value="Agent">Agent</option><option value="Skill">Skill</option></select></label>
      <label><span>{tr("controls.paths", {}, $locale)}</span><input bind:value={custom.source_path} readonly={controlState.store_sandbox} required placeholder={controlState.store_sandbox ? tr("path.folderRequired", {}, $locale) : "/Users/name/.tool/skills"} autocomplete="off" spellcheck="false" /></label>
      {#if controlState.store_sandbox}<button type="button" class="secondary-button" onclick={chooseFolder}><FolderOpen size={15} />{tr("path.chooseFolder", {}, $locale)}</button>{/if}
      <button class="primary-button">{tr("common.save", {}, $locale)}</button>
    </form>
  </div>
{/if}
