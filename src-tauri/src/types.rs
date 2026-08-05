use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Location {
    pub path: String,
    #[serde(default)]
    pub source_class: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub project_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Snapshot {
    #[serde(default)]
    pub resource_id: String,
    #[serde(default)]
    pub object_hash: String,
    #[serde(default)]
    pub copy_status: String,
    #[serde(default)]
    pub source_access: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Variant {
    pub sha256: String,
    #[serde(default)]
    pub locations: Vec<Location>,
    #[serde(default)]
    pub states: Vec<String>,
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReadyVersion {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub canonical_path: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetGroup {
    pub logical_id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub usage: String,
    #[serde(default)]
    pub lifecycle_state: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub provenance: Vec<String>,
    #[serde(default)]
    pub variants: Vec<Variant>,
    #[serde(default)]
    pub detail_note: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub ready_versions: Vec<ReadyVersion>,
    #[serde(default)]
    pub snapshot_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Registry {
    pub schema_version: u64,
    #[serde(default)]
    pub policy: serde_json::Value,
    pub physical_entry_count: u64,
    pub logical_asset_count: u64,
    pub unique_hash_count: u64,
    #[serde(default)]
    pub groups: Vec<AssetGroup>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventoryEnvelope {
    pub registry: Registry,
    pub source_path: String,
    pub source_modified_at: String,
    pub source_size_bytes: u64,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySource {
    pub source_path: String,
    pub source_modified_at: String,
    pub source_size_bytes: u64,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshStep {
    pub name: String,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshResult {
    pub mode: String,
    pub status: String,
    pub steps: Vec<RefreshStep>,
    pub registry_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub destination: String,
    pub format: String,
    pub export_mode: String,
    pub asset_count: usize,
    pub agent_count: usize,
    pub skill_count: usize,
    pub relationship_count: usize,
    pub file_count: usize,
    pub warning_count: usize,
    pub skipped_count: usize,
    pub sha256: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelationEvidence {
    pub reference: String,
    pub source_path: String,
    pub rule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompanionSkill {
    pub logical_id: String,
    pub name: String,
    #[serde(default)]
    pub evidence: Vec<RelationEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentSkillResolution {
    pub agent_logical_id: String,
    pub agent_name: String,
    #[serde(default)]
    pub skills: Vec<CompanionSkill>,
    #[serde(default)]
    pub unresolved_references: Vec<String>,
    #[serde(default)]
    pub inspected_paths: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExportRelationship {
    pub agent_logical_id: String,
    pub skill_logical_id: String,
    pub source: String,
    #[serde(default)]
    pub evidence: Vec<RelationEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub endpoint: String,
    /// 默认用于最小推理验真的模型。
    pub model: String,
    /// 该接口中由用户选择纳入统一管理的模型目录。
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub api_key_env: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub credential_stored: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelTestResult {
    pub profile_id: String,
    pub mode: String,
    pub status: String,
    pub latency_ms: u128,
    pub summary: String,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolBindingPreview {
    pub id: String,
    pub tool: String,
    pub kind: String,
    pub source_path: String,
    pub target_path: String,
    pub source_state: String,
    pub entry_count: usize,
    pub detected: bool,
    pub status: String,
    pub can_apply: bool,
    pub requires_migration: bool,
    pub message: String,
    pub custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomToolBinding {
    pub id: String,
    pub tool: String,
    pub kind: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingReceipt {
    pub id: String,
    pub binding_id: String,
    pub source_path: String,
    pub target_path: String,
    pub prior_state: String,
    #[serde(default)]
    pub backup_path: String,
    pub applied_at: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlCenterState {
    pub profiles: Vec<ModelProfile>,
    pub bindings: Vec<ToolBindingPreview>,
    pub receipts: Vec<BindingReceipt>,
    pub unified_root: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditCheck {
    pub name: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetAuditReport {
    pub logical_id: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub score: u8,
    pub checks: Vec<AuditCheck>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetAuditResult {
    pub scope: String,
    pub truth_level: String,
    pub total: usize,
    pub usable: usize,
    pub needs_attention: usize,
    pub blocked: usize,
    pub duration_ms: u128,
    pub checked_at: String,
    pub reports: Vec<AssetAuditReport>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetLicenseReceipt {
    pub decision: String,
    pub evidence: String,
    pub confirmed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetVerificationReceipt {
    pub sha256: String,
    pub evidence: String,
    pub truth_level: String,
    pub status: String,
    pub verified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationDraftReceipt {
    pub id: String,
    pub source_path: String,
    pub source_sha256: String,
    pub working_file: String,
    pub plan_path: String,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub applied_at: String,
    #[serde(default)]
    pub output_sha256: String,
    #[serde(default)]
    pub backup_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetGovernanceRecord {
    pub logical_id: String,
    #[serde(default)]
    pub selected_sha256: String,
    #[serde(default)]
    pub selected_at: String,
    #[serde(default)]
    pub license: Option<AssetLicenseReceipt>,
    #[serde(default)]
    pub verification: Option<AssetVerificationReceipt>,
    #[serde(default)]
    pub drafts: Vec<OptimizationDraftReceipt>,
    #[serde(default)]
    pub pending_refresh: bool,
    #[serde(default)]
    pub updated_at: String,
}
