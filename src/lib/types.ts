export type AssetKind = "Agent" | "Skill" | string;

export interface Location {
  path: string;
  source_class?: string;
  project?: string;
  project_root?: string;
}

export interface Snapshot {
  resource_id?: string;
  object_hash?: string;
  copy_status?: string;
  source_access?: string;
}

export interface Variant {
  sha256: string;
  locations: Location[];
  states?: string[];
  snapshots?: Snapshot[];
}

export interface ReadyVersion {
  id?: string;
  version?: string;
  path?: string;
  canonical_path?: string;
  sha256?: string;
  state?: string;
  status?: string;
}

export interface AssetGroup {
  logical_id: string;
  kind: AssetKind;
  name: string;
  description?: string;
  usage?: string;
  lifecycle_state?: string;
  tools?: string[];
  projects?: string[];
  provenance?: string[];
  variants: Variant[];
  detail_note?: string;
  license?: string;
  ready_versions?: ReadyVersion[];
  snapshot_count?: number;
}

export interface Registry {
  schema_version: number;
  policy?: Record<string, unknown>;
  physical_entry_count: number;
  logical_asset_count: number;
  unique_hash_count: number;
  groups: AssetGroup[];
  updated_at: string;
}

export interface InventoryEnvelope {
  registry: Registry;
  source_path: string;
  source_modified_at: string;
  source_size_bytes: number;
  source_fingerprint: string;
}

export interface InventorySource {
  source_path: string;
  source_modified_at: string;
  source_size_bytes: number;
  source_fingerprint: string;
}

export interface RefreshResult {
  mode: "local" | "full";
  status: "success" | "blocked";
  steps: Array<{
    name: string;
    status: "success" | "failed" | "timeout";
    summary: string;
  }>;
  registry_updated_at?: string;
}

export type ExportFormat = "json" | "markdown" | "bundle";
export type ExportMode = "selection" | "agents_only" | "skills_only" | "agent_with_skills";

export interface RelationEvidence {
  reference: string;
  source_path: string;
  rule: string;
}

export interface CompanionSkill {
  logical_id: string;
  name: string;
  evidence: RelationEvidence[];
}

export interface AgentSkillResolution {
  agent_logical_id: string;
  agent_name: string;
  skills: CompanionSkill[];
  unresolved_references: string[];
  inspected_paths: string[];
  warnings: string[];
}

export interface ExportResult {
  destination: string;
  format: ExportFormat;
  export_mode: ExportMode;
  asset_count: number;
  agent_count: number;
  skill_count: number;
  relationship_count: number;
  file_count: number;
  warning_count: number;
  skipped_count: number;
  sha256: string;
  warnings: string[];
}

export interface ExportHistoryEntry extends ExportResult {
  completed_at: string;
}

export interface ProjectSummary {
  name: string;
  agents: number;
  skills: number;
  assets: number;
  ready: number;
}

export interface ModelProfile {
  id: string;
  name: string;
  provider: "ollama" | "openai_compatible";
  endpoint: string;
  model: string;
  models: string[];
  api_key_env: string;
  enabled: boolean;
  credential_stored: boolean;
}

export interface ModelOption {
  id: string;
  label: string;
}

export interface ModelTestResult {
  profile_id: string;
  mode: "connectivity" | "inference";
  status: string;
  latency_ms: number;
  summary: string;
  checked_at: string;
}

export interface ToolBindingPreview {
  id: string;
  tool: string;
  kind: "Agent" | "Skill";
  source_path: string;
  target_path: string;
  source_state: string;
  entry_count: number;
  detected: boolean;
  status: string;
  can_apply: boolean;
  requires_migration: boolean;
  message: string;
  custom: boolean;
}

export interface CustomToolBinding {
  id: string;
  tool: string;
  kind: "Agent" | "Skill";
  source_path: string;
  security_bookmark?: string;
}

export interface BindingReceipt {
  id: string;
  binding_id: string;
  source_path: string;
  target_path: string;
  prior_state: string;
  backup_path: string;
  applied_at: string;
  status: "applied" | "restored";
}

export interface ControlCenterState {
  profiles: ModelProfile[];
  bindings: ToolBindingPreview[];
  receipts: BindingReceipt[];
  unified_root: string;
  store_sandbox: boolean;
}

export interface AuditCheck {
  name: string;
  status: "pass" | "warn" | "fail" | "skipped";
  evidence: string;
}

export interface AssetAuditReport {
  logical_id: string;
  name: string;
  kind: AssetKind;
  status: "usable" | "needs_attention" | "blocked";
  score: number;
  checks: AuditCheck[];
  suggestions: string[];
}

export interface AssetAuditResult {
  scope: "all" | "selected";
  truth_level: "static_light" | "static_deep";
  total: number;
  usable: number;
  needs_attention: number;
  blocked: number;
  duration_ms: number;
  checked_at: string;
  reports: AssetAuditReport[];
  truncated: boolean;
}

export interface AssetLicenseReceipt {
  decision: string;
  evidence: string;
  confirmed_at: string;
}

export interface AssetVerificationReceipt {
  sha256: string;
  evidence: string;
  truth_level: "local_static_plus_user_evidence" | string;
  status: "verified_local" | string;
  verified_at: string;
}

export interface OptimizationDraftReceipt {
  id: string;
  source_path: string;
  source_sha256: string;
  working_file: string;
  plan_path: string;
  status: "draft" | "applied" | string;
  created_at: string;
  applied_at: string;
  output_sha256: string;
  backup_path: string;
}

export interface AssetGovernanceRecord {
  logical_id: string;
  selected_sha256: string;
  selected_at: string;
  license?: AssetLicenseReceipt;
  verification?: AssetVerificationReceipt;
  drafts: OptimizationDraftReceipt[];
  pending_refresh: boolean;
  updated_at: string;
}

export type ViewId = "overview" | "assets" | "projects" | "updates" | "controls" | "exports";

export interface AssetDefinition {
  logical_id: string;
  sha256: string;
  path: string;
  content: string;
  byte_count: number;
  line_count: number;
  estimated_tokens: number;
}

export interface DefinitionSearchResult {
  matches: Array<{ logical_id: string; sha256: string; path: string; snippet: string }>;
  scanned_files: number;
  skipped_files: number;
  truncated: boolean;
}
