import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

const demoHome = process.argv[2];
if (!demoHome) {
  throw new Error("Usage: node marketing/generate-demo-home.mjs <isolated-home>");
}

const agents = [
  ["Code Review Agent", "Reviews changes for correctness, maintainability, and release risk.", ["Codex", "Claude"], ["Atlas Demo", "Beacon API"]],
  ["API Test Agent", "Builds contract-driven API checks and actionable failure reports.", ["Codex", "Cursor"], ["Beacon API"]],
  ["Documentation Agent", "Keeps product and engineering documentation aligned with shipped behavior.", ["Claude", "Gemini CLI"], ["Atlas Demo", "Aurora CLI"]],
  ["Release Guard", "Runs clean-release, privacy, and packaging gates before distribution.", ["Codex", "Trae"], ["Nova Studio", "Aurora CLI"]],
  ["Accessibility Agent", "Checks interface semantics, keyboard paths, and inclusive language.", ["Claude", "Windsurf"], ["Nova Studio"]],
  ["Security Review Agent", "Finds secret exposure, unsafe defaults, and permission boundary risks.", ["Codex", "OpenCode"], ["Beacon API", "Nova Studio"]],
  ["Localization Agent", "Audits locale coverage, missing messages, and RTL readiness.", ["Gemini CLI", "Trae"], ["Atlas Demo"]],
  ["Performance Agent", "Profiles hot paths and proposes measurable resource optimizations.", ["Cursor", "Windsurf"], ["Aurora CLI", "Nova Studio"]]
];

const skills = [
  ["Rust Quality", "Run focused Rust quality gates and explain high-value findings.", ["Codex"], ["Aurora CLI"]],
  ["Svelte UI", "Create accessible Svelte interfaces with clear responsive behavior.", ["Claude", "Cursor"], ["Nova Studio"]],
  ["API Contract", "Design and verify explicit API contracts with compatibility rules.", ["Codex", "OpenCode"], ["Beacon API"]],
  ["Browser Testing", "Exercise critical user journeys and preserve visual evidence.", ["Trae", "Windsurf"], ["Atlas Demo", "Nova Studio"]],
  ["Release Packaging", "Build reproducible release artifacts and validate their contents.", ["Codex"], ["Aurora CLI", "Nova Studio"]],
  ["Prompt Design", "Refine operational prompts around evidence, scope, and success criteria.", ["Claude", "Gemini CLI"], ["Atlas Demo"]],
  ["Accessibility Check", "Audit keyboard, screen reader, contrast, and focus behavior.", ["Claude"], ["Nova Studio"]],
  ["Secret Scan", "Detect credentials and private material before publishing.", ["Codex", "Trae"], ["Beacon API", "Aurora CLI"]],
  ["Model Routing", "Route tasks across local and hosted models with explicit fallbacks.", ["OpenCode", "Gemini CLI"], ["Atlas Demo", "Beacon API"]],
  ["Performance Audit", "Measure CPU, memory, I/O, and bundle costs before optimization.", ["Cursor", "Windsurf"], ["Nova Studio"]],
  ["Localization", "Keep interface copy complete across eleven supported locales.", ["Claude", "Trae"], ["Atlas Demo"]],
  ["Architecture Notes", "Capture durable technical decisions without duplicating transient logs.", ["Codex", "Claude"], ["Beacon API"]],
  ["Data Migration", "Plan reversible schema and configuration migrations.", ["OpenCode", "Cursor"], ["Beacon API", "Aurora CLI"]],
  ["Product Discovery", "Turn observed needs into testable product opportunities.", ["Gemini CLI", "Claude"], ["Atlas Demo"]],
  ["CLI Experience", "Improve command help, errors, output structure, and recovery paths.", ["Codex", "Windsurf"], ["Aurora CLI"]],
  ["Privacy Review", "Map data flows and validate privacy disclosures against behavior.", ["Trae", "Claude"], ["Nova Studio"]]
];

const appRoot = join(demoHome, "Library", "Application Support", "app.lingzhan.agent-skill-hub");
const cacheRoot = join(appRoot, "cache");
const definitionRoot = join(demoHome, "Demo Workspace", "Definitions");
const now = "2026-08-05T14:30:00+08:00";

await mkdir(cacheRoot, { recursive: true });
await mkdir(definitionRoot, { recursive: true });

const groups = [];
const governance = [];
for (const [index, item] of [...agents, ...skills].entries()) {
  const [name, description, tools, projects] = item;
  const kind = index < agents.length ? "Agent" : "Skill";
  const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  const logicalId = `demo-${kind.toLowerCase()}-${slug}`;
  const versions = index % 7 === 1 || index % 9 === 0 ? ["1.0.0", "1.1.0"] : ["1.0.0"];
  const variants = [];

  for (const [versionIndex, version] of versions.entries()) {
    const location = kind === "Skill"
      ? join(definitionRoot, tools[versionIndex % tools.length], `${slug}-${version}`, "SKILL.md")
      : join(definitionRoot, tools[versionIndex % tools.length], `${slug}-${version}.md`);
    const content = kind === "Skill"
      ? `---\nname: ${name}\ndescription: ${description}\nversion: ${version}\nlicense: ${index % 4 === 0 ? "Apache-2.0" : "MIT"}\n---\n\n# ${name}\n\n## Purpose\n\n${description}\n\n## Workflow\n\n1. Inspect the explicit task scope.\n2. Run the narrowest relevant checks.\n3. Report evidence, limits, and next actions.\n`
      : `# ${name}\n\nVersion: ${version}\nLicense: ${index % 3 === 0 ? "Apache-2.0" : "MIT"}\n\n${description}\n\n## Operating contract\n\n- Read the requested scope before acting.\n- Preserve user-owned changes.\n- Verify outcomes with concrete evidence.\n`;
    await mkdir(join(location, ".."), { recursive: true });
    await writeFile(location, content, "utf8");
    const sha256 = createHash("sha256").update(content).digest("hex");
    variants.push({
      sha256,
      locations: [{
        path: kind === "Skill" ? join(location, "..") : location,
        source_class: tools[versionIndex % tools.length],
        project: projects[0],
        project_root: join(demoHome, "Demo Workspace", "Projects", projects[0])
      }],
      states: versionIndex === versions.length - 1 ? ["candidate", "local"] : ["archived", "local"],
      snapshots: []
    });
  }

  const selected = variants.at(-1);
  const ready = index % 5 !== 3;
  groups.push({
    logical_id: logicalId,
    kind,
    name,
    description,
    usage: kind === "Agent" ? "Project automation and focused review" : "Reusable workflow capability",
    lifecycle_state: ready ? "verified_local" : "needs_review",
    tools,
    projects,
    provenance: ["Synthetic App Store demonstration data"],
    variants,
    detail_note: "",
    license: index % 4 === 0 ? "Apache-2.0" : "MIT",
    ready_versions: ready ? [{
      id: `${logicalId}-ready`,
      version: versions.at(-1),
      path: selected.locations[0].path,
      canonical_path: selected.locations[0].path,
      sha256: selected.sha256,
      state: "verified_local",
      status: "ready"
    }] : [],
    snapshot_count: ready ? 1 : 0
  });

  if (ready && index < 12) {
    governance.push({
      logical_id: logicalId,
      selected_sha256: selected.sha256,
      selected_at: now,
      license: {
        decision: "Approved for this local demo workflow",
        evidence: "Synthetic fixture with a declared license and no private content.",
        confirmed_at: now
      },
      verification: {
        sha256: selected.sha256,
        evidence: "Structure, integrity, and local source availability checks passed.",
        truth_level: "local_static_plus_user_evidence",
        status: "verified_local",
        verified_at: now
      },
      drafts: [],
      pending_refresh: false,
      updated_at: now
    });
  }
}

const hashes = new Set(groups.flatMap((group) => group.variants.map((variant) => variant.sha256)));
const registry = {
  schema_version: 2,
  policy: {
    distribution: "marketing-demo",
    bundled_agent_skill_count: 0,
    discovery: "isolated-synthetic-fixture",
    public_cache_epoch: 2,
    truth_boundary: "Synthetic data for App Store imagery; not shipped with the application."
  },
  physical_entry_count: groups.reduce((sum, group) => sum + group.variants.length, 0),
  logical_asset_count: groups.length,
  unique_hash_count: hashes.size,
  groups,
  updated_at: now
};

const controlCenter = {
  profiles: [
    {
      id: "local-models",
      name: "Local Models",
      provider: "ollama",
      endpoint: "http://127.0.0.1:11434",
      model: "qwen3:8b",
      models: ["qwen3:8b", "deepseek-r1:8b", "gemma3:12b"],
      api_key_env: "",
      enabled: true,
      credential_stored: false
    },
    {
      id: "openai-compatible",
      name: "OpenAI Compatible",
      provider: "openai_compatible",
      endpoint: "https://api.example.com/v1",
      model: "gpt-5",
      models: ["gpt-5", "gpt-5-mini", "o4-mini"],
      api_key_env: "MODEL_PROVIDER_KEY",
      enabled: true,
      credential_stored: false
    },
    {
      id: "team-gateway",
      name: "Team Gateway",
      provider: "openai_compatible",
      endpoint: "https://models.example.com/v1",
      model: "coder-large",
      models: ["claude-sonnet", "gemini-pro", "coder-large"],
      api_key_env: "TEAM_MODEL_KEY",
      enabled: true,
      credential_stored: false
    }
  ],
  custom_bindings: []
};

await Promise.all([
  writeFile(join(cacheRoot, "registry.json"), JSON.stringify(registry, null, 2), "utf8"),
  writeFile(join(appRoot, "control-center.json"), JSON.stringify(controlCenter, null, 2), "utf8"),
  writeFile(join(appRoot, "asset-governance.json"), JSON.stringify({ records: governance }, null, 2), "utf8")
]);

console.log(JSON.stringify({ demoHome, assets: groups.length, variants: registry.physical_entry_count, verified: governance.length }));
