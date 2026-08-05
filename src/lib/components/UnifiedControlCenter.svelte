<script lang="ts">
  import { onMount } from "svelte";
  import { Bot, Cable, FolderSync, ShieldCheck } from "@lucide/svelte";
  import { loadControlCenter } from "../api";
  import type { AssetGroup, ControlCenterState } from "../types";
  import ModelAccessPanel from "./ModelAccessPanel.svelte";
  import ToolPathPanel from "./ToolPathPanel.svelte";
  import VerificationPanel from "./VerificationPanel.svelte";
  import { locale, tr } from "../i18n";

  let { groups, openAsset }: { groups: AssetGroup[]; openAsset: (asset: AssetGroup) => void } = $props();
  let active = $state<"models" | "paths" | "audit">("models");
  let controlState = $state<ControlCenterState | null>(null);
  let loading = $state(true);
  let error = $state("");

  async function reload() {
    loading = true;
    try {
      controlState = await loadControlCenter();
      error = "";
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      loading = false;
    }
  }

  function reportError(reason: unknown) {
    error = reason instanceof Error ? reason.message : String(reason);
  }

  onMount(reload);
</script>

<section class="compact-heading control-heading">
  <div>
    <span class="eyebrow">UNIFIED RUNTIME CONTROL</span>
    <h1>{tr("controls.title", {}, $locale)}</h1>
    <p>{tr("controls.subtitle", {}, $locale)}</p>
  </div>
  <div class="control-safety"><ShieldCheck size={17} /><span>{tr("controls.safety", {}, $locale)}</span></div>
</section>

<nav class="control-tabs" aria-label={tr("nav.controls", {}, $locale)}>
  <button class:active={active === "models"} onclick={() => (active = "models")}><Cable size={17} /><span>{tr("controls.models", {}, $locale)}</span></button>
  <button class:active={active === "paths"} onclick={() => (active = "paths")}><FolderSync size={17} /><span>{tr("controls.paths", {}, $locale)}</span></button>
  <button class:active={active === "audit"} onclick={() => (active = "audit")}><Bot size={17} /><span>{tr("controls.audit", {}, $locale)}</span></button>
</nav>

{#if error}<div class="control-error">{error}<button onclick={() => (error = "")}>{tr("common.close", {}, $locale)}</button></div>{/if}
{#if loading && !controlState}
  <div class="control-loading">{tr("common.loading", {}, $locale)}</div>
{:else if active === "models" && controlState}
  <ModelAccessPanel {controlState} setState={(next) => (controlState = next)} {reportError} />
{:else if active === "paths" && controlState}
  <ToolPathPanel {controlState} {reload} {reportError} />
{:else if active === "audit"}
  <VerificationPanel {groups} {openAsset} {reportError} />
{/if}
