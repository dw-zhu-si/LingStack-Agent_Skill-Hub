<script lang="ts">
  import { CheckCircle2, Download, FlaskConical, KeyRound, Plus, Save, Trash2, Wifi } from "@lucide/svelte";
  import { clearModelCredential, deleteModelProfile, listModelOptions, saveModelProfile, testModelProfile } from "../api";
  import type { ControlCenterState, ModelOption, ModelProfile, ModelTestResult } from "../types";
  import { locale, tr } from "../i18n";

  let { controlState, setState, reportError }: {
    controlState: ControlCenterState;
    setState: (state: ControlCenterState) => void;
    reportError: (reason: unknown) => void;
  } = $props();
  let editing = $state<ModelProfile | null>(null);
  let busy = $state("");
  let results = $state<Record<string, ModelTestResult>>({});
  let secret = $state("");
  let modelOptions = $state<ModelOption[]>([]);
  let loadingModels = $state(false);

  function normalizeEndpoint(value: string) {
    const trimmed = value.trim().replaceAll("，", ",");
    return trimmed.replace(/:(\d[\d,]*)(?=\/|$)/, (_match, port: string) => `:${port.replaceAll(",", "")}`);
  }

  function prepareEndpoint() {
    if (!editing) return;
    editing.endpoint = normalizeEndpoint(editing.endpoint);
    try {
      const parsed = new URL(editing.endpoint);
      if (!["http:", "https:"].includes(parsed.protocol)) throw new Error();
    } catch {
      throw new Error(tr("model.endpoint", {}, $locale));
    }
  }

  function selectedModels(profile: ModelProfile) {
    return profile.models?.length ? profile.models : (profile.model ? [profile.model] : []);
  }

  function toggleModel(id: string, checked: boolean) {
    if (!editing) return;
    const next = new Set(selectedModels(editing));
    if (checked) next.add(id); else next.delete(id);
    editing.models = [...next].sort();
    if (!editing.models.includes(editing.model)) editing.model = editing.models[0] ?? "";
  }

  function newProfile() {
    editing = {
      id: `model-${Date.now()}`,
      name: "OpenAI Compatible",
      provider: "openai_compatible",
      endpoint: "https://api.openai.com/v1",
      model: "",
      models: [],
      api_key_env: "OPENAI_API_KEY",
      enabled: true,
      credential_stored: false
    };
    secret = "";
    modelOptions = [];
  }

  function editProfile(profile: ModelProfile) {
    editing = { ...profile, models: selectedModels(profile) };
    secret = "";
    modelOptions = profile.model ? [{ id: profile.model, label: profile.model }] : [];
  }

  async function saveProfile() {
    if (!editing) return;
    busy = editing.id;
    try {
      prepareEndpoint();
      if (editing.models.length === 0 && editing.model.trim()) editing.models = [editing.model.trim()];
      setState(await saveModelProfile(editing, secret));
      secret = "";
      editing = null;
    } catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function fetchModels() {
    if (!editing) return;
    loadingModels = true;
    try {
      prepareEndpoint();
      modelOptions = await listModelOptions(editing, secret);
      if (modelOptions.length === 0) throw new Error(tr("model.notSelected", {}, $locale));
      editing.models = modelOptions.map((option) => option.id);
      if (!modelOptions.some((option) => option.id === editing?.model)) editing.model = modelOptions[0].id;
    } catch (reason) { reportError(reason); }
    finally { loadingModels = false; }
  }

  async function autoFetchModels() {
    if (!editing || loadingModels || modelOptions.length > 0) return;
    if (editing.provider !== "ollama" && !secret && !editing.credential_stored) return;
    await fetchModels();
  }

  async function removeProfile(profile: ModelProfile) {
    if (!window.confirm(`${tr("common.delete", {}, $locale)}: ${profile.name}?`)) return;
    busy = profile.id;
    try { setState(await deleteModelProfile(profile.id)); }
    catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function clearCredential(profile: ModelProfile) {
    if (!window.confirm(`${tr("common.delete", {}, $locale)}: ${profile.name}?`)) return;
    busy = profile.id;
    try { setState(await clearModelCredential(profile.id)); }
    catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }

  async function test(profile: ModelProfile, inference: boolean) {
    if (inference && !window.confirm(`${tr("model.inference", {}, $locale)}?`)) return;
    busy = profile.id;
    try {
      results = { ...results, [profile.id]: await testModelProfile(profile.id, inference) };
    } catch (reason) { reportError(reason); }
    finally { busy = ""; }
  }
</script>

<section class="control-grid model-control">
  <article class="panel control-main">
    <header class="control-panel-heading">
      <div><span class="section-kicker">MODEL PROFILES</span><h2>{tr("model.profiles", {}, $locale)}</h2></div>
      <button class="secondary-button small" onclick={newProfile}><Plus size={15} />{tr("model.add", {}, $locale)}</button>
    </header>
    <p class="control-note">{tr("controls.safety", {}, $locale)}</p>
    <div class="model-list">
      {#each controlState.profiles as profile}
        <article>
          <div class="model-mark">{profile.provider === "ollama" ? "OL" : "AI"}</div>
          <div class="model-copy"><strong>{profile.name}</strong><small>{tr("model.count", { count: selectedModels(profile).length }, $locale)} · {tr("model.default", {}, $locale)} {profile.model || tr("model.notSelected", {}, $locale)}</small><code>{profile.endpoint}</code></div>
          <span class:off={!profile.enabled} class="binding-state">{tr(profile.enabled ? "common.enabled" : "common.disabled", {}, $locale)}</span>
          <div class="model-actions">
            <button disabled={busy === profile.id} onclick={() => test(profile, false)}><Wifi size={14} />{tr("model.connect", {}, $locale)}</button>
            <button disabled={busy === profile.id} onclick={() => test(profile, true)}><FlaskConical size={14} />{tr("model.inference", {}, $locale)}</button>
            <button onclick={() => editProfile(profile)}>{tr("common.edit", {}, $locale)}</button>
            {#if profile.credential_stored}<button class="danger-text" title={tr("common.clear", {}, $locale)} aria-label={`${tr("common.clear", {}, $locale)} ${profile.name}`} disabled={busy === profile.id} onclick={() => clearCredential(profile)}><KeyRound size={14} /></button>{/if}
            {#if profile.id !== "ollama-local"}<button class="danger-text" onclick={() => removeProfile(profile)}><Trash2 size={14} /></button>{/if}
          </div>
          {#if results[profile.id]}<div class="model-result"><CheckCircle2 size={14} />{results[profile.id].summary} · {results[profile.id].latency_ms}ms</div>{/if}
        </article>
      {/each}
    </div>
  </article>
  <aside class="panel control-aside">
    <div class="control-panel-heading"><div><span class="section-kicker">BOUNDARY</span><h2>{tr("status.truthBoundary", {}, $locale)}</h2></div></div>
    <ul class="boundary-list">
      <li><strong>{tr("model.connect", {}, $locale)}</strong><span>{tr("controls.safety", {}, $locale)}</span></li>
      <li><strong>{tr("model.inference", {}, $locale)}</strong><span>{tr("common.confirm", {}, $locale)}</span></li>
      <li><strong>{tr("model.apiKey", {}, $locale)}</strong><span>{tr("controls.safety", {}, $locale)}</span></li>
    </ul>
  </aside>
</section>

{#if editing}
  <div class="control-modal-backdrop" role="presentation">
    <form class="control-modal" autocomplete="off" onsubmit={(event) => { event.preventDefault(); saveProfile(); }}>
      <header><div><span class="section-kicker">MODEL PROFILE</span><h2>{tr("model.profile", {}, $locale)}</h2></div><button type="button" onclick={() => (editing = null)}>{tr("common.cancel", {}, $locale)}</button></header>
      <label><span>{tr("model.displayName", {}, $locale)}</span><input bind:value={editing.name} required autocomplete="off" /></label>
      <label><span>{tr("model.interfaceType", {}, $locale)}</span><select bind:value={editing.provider}><option value="openai_compatible">OpenAI</option><option value="ollama">Ollama</option></select></label>
      <label><span>{tr("model.endpoint", {}, $locale)}</span><input bind:value={editing.endpoint} type="text" inputmode="url" required autocomplete="off" spellcheck="false" placeholder="http://127.0.0.1:11435/v1" onblur={() => { if (editing) editing.endpoint = normalizeEndpoint(editing.endpoint); void autoFetchModels(); }} /></label>
      <div class="model-picker-heading"><span>{tr("model.catalog", {}, $locale)}</span><button type="button" disabled={loadingModels} onclick={fetchModels}><Download size={14} />{tr(loadingModels ? "model.fetching" : "model.fetch", {}, $locale)}</button></div>
      {#if modelOptions.length > 0}
        <div class="model-catalog-summary">{tr("model.count", { count: modelOptions.length }, $locale)} · {tr("common.items", { count: selectedModels(editing).length }, $locale)} {tr("common.selected", {}, $locale)}</div>
        <div class="model-catalog">
          {#each modelOptions as option}
            <label><input type="checkbox" checked={selectedModels(editing).includes(option.id)} onchange={(event) => toggleModel(option.id, event.currentTarget.checked)} /><span>{option.label}</span></label>
          {/each}
        </div>
        <label><span>{tr("model.defaultModel", {}, $locale)}</span><select class="model-picker" bind:value={editing.model} required>{#each selectedModels(editing) as id}<option value={id}>{modelOptions.find((option) => option.id === id)?.label ?? id}</option>{/each}</select></label>
      {:else}
        <input class="model-picker" bind:value={editing.model} placeholder={tr("model.fetch", {}, $locale)} required autocomplete="off" spellcheck="false" />
      {/if}
      <label><span>{tr("model.apiKey", {}, $locale)}</span><input type="password" bind:value={secret} autocomplete="new-password" spellcheck="false" onblur={() => void autoFetchModels()} /></label>
      <label><span>{tr("model.envName", {}, $locale)}</span><input bind:value={editing.api_key_env} placeholder="OPENAI_API_KEY" pattern="[A-Z_][A-Z0-9_]*" autocomplete="off" spellcheck="false" /></label>
      <label class="inline-check"><input type="checkbox" bind:checked={editing.enabled} /><span>{tr("common.enabled", {}, $locale)}</span></label>
      <button class="primary-button" disabled={busy === editing.id}><Save size={15} />{tr("model.save", {}, $locale)}</button>
    </form>
  </div>
{/if}
