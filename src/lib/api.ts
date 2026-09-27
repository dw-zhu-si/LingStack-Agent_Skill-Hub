import { invoke } from "@tauri-apps/api/core";
import type {
  AgentSkillResolution,
  AssetAuditResult,
  BindingReceipt,
  ControlCenterState,
  CustomToolBinding,
  ExportFormat,
  ExportMode,
  ExportResult,
  InventoryEnvelope,
  InventorySource,
  ModelProfile,
  AssetGovernanceRecord,
  ModelOption,
  ModelTestResult,
  RefreshResult
} from "./types";

const DEMO_REGISTRY_PATH = "/demo-registry.json";

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function loadInventory(): Promise<InventoryEnvelope> {
  if (isTauri()) {
    return invoke<InventoryEnvelope>("load_inventory");
  }

  const response = await fetch(DEMO_REGISTRY_PATH);
  if (!response.ok) {
    throw new Error("浏览器预览没有找到 demo-registry.json，请使用 Tauri 启动完整应用。");
  }
  const registry = await response.json();
  return {
    registry,
    source_path: "浏览器预览数据",
    source_modified_at: registry.updated_at,
    source_size_bytes: 0,
    source_fingerprint: "demo"
  };
}

export async function inspectInventory(): Promise<InventorySource> {
  if (isTauri()) {
    return invoke<InventorySource>("inspect_inventory");
  }

  return {
    source_path: "浏览器预览数据",
    source_modified_at: "demo",
    source_size_bytes: 0,
    source_fingerprint: "demo"
  };
}

export async function refreshInventory(mode: "local" | "full"): Promise<RefreshResult> {
  if (!isTauri()) {
    throw new Error("深度刷新需要在桌面应用中运行。");
  }
  return invoke<RefreshResult>("refresh_inventory", { mode });
}

export async function exportAssets(
  logicalIds: string[],
  format: ExportFormat,
  destination: string,
  exportMode: ExportMode
): Promise<ExportResult> {
  if (!isTauri()) {
    throw new Error("文件导出需要在桌面应用中运行。");
  }
  return invoke<ExportResult>("export_assets", {
    logicalIds,
    format,
    destination,
    exportMode
  });
}

export async function resolveAgentSkills(agentLogicalId: string): Promise<AgentSkillResolution> {
  if (!isTauri()) {
    throw new Error("Agent 配合 Skill 分析需要在桌面应用中运行。");
  }
  return invoke<AgentSkillResolution>("resolve_agent_skills", { agentLogicalId });
}

export async function loadControlCenter(): Promise<ControlCenterState> {
  if (!isTauri()) throw new Error("统一控制中心需要在桌面应用中运行。");
  return invoke<ControlCenterState>("load_control_center");
}

export async function saveModelProfile(profile: ModelProfile, secret = ""): Promise<ControlCenterState> {
  if (!isTauri()) throw new Error("模型配置需要在桌面应用中保存。");
  return invoke<ControlCenterState>("save_model_profile", { profile, secret });
}

export async function listModelOptions(profile: ModelProfile, secret = ""): Promise<ModelOption[]> {
  if (!isTauri()) throw new Error("模型列表需要在桌面应用中拉取。");
  return invoke<ModelOption[]>("list_model_options", { profile, secret });
}

export async function deleteModelProfile(id: string): Promise<ControlCenterState> {
  if (!isTauri()) throw new Error("模型配置需要在桌面应用中修改。");
  return invoke<ControlCenterState>("delete_model_profile", { id });
}

export async function clearModelCredential(id: string): Promise<ControlCenterState> {
  if (!isTauri()) throw new Error("模型密钥需要在桌面应用中清除。");
  return invoke<ControlCenterState>("clear_model_credential", {
    id,
    confirmation: `CLEAR_CREDENTIAL:${id}`
  });
}

export async function saveCustomBinding(binding: CustomToolBinding): Promise<ControlCenterState> {
  if (!isTauri()) throw new Error("自定义工具入口需要在桌面应用中保存。");
  return invoke<ControlCenterState>("save_custom_binding", { binding });
}

export async function deleteCustomBinding(id: string): Promise<ControlCenterState> {
  if (!isTauri()) throw new Error("自定义工具入口需要在桌面应用中修改。");
  return invoke<ControlCenterState>("delete_custom_binding", { id });
}

export async function testModelProfile(id: string, inference: boolean): Promise<ModelTestResult> {
  if (!isTauri()) throw new Error("模型验真需要在桌面应用中运行。");
  return invoke<ModelTestResult>("test_model_profile", { id, inference });
}

export async function applyToolBinding(
  bindingId: string,
  migrateExisting: boolean
): Promise<BindingReceipt> {
  if (!isTauri()) throw new Error("工具路径切换需要在桌面应用中运行。");
  return invoke<BindingReceipt>("apply_tool_binding", {
    bindingId,
    migrateExisting,
    confirmation: `APPLY:${bindingId}`
  });
}

export async function restoreToolBinding(receiptId: string): Promise<BindingReceipt> {
  if (!isTauri()) throw new Error("工具路径恢复需要在桌面应用中运行。");
  return invoke<BindingReceipt>("restore_tool_binding", {
    receiptId,
    confirmation: `RESTORE:${receiptId}`
  });
}

export async function runAssetAudit(logicalIds: string[], deep: boolean): Promise<AssetAuditResult> {
  if (!isTauri()) throw new Error("资产验真需要在桌面应用中运行。");
  return invoke<AssetAuditResult>("run_asset_audit", { logicalIds, deep });
}

export async function loadAssetGovernance(logicalIds: string[] = []): Promise<AssetGovernanceRecord[]> {
  if (!isTauri()) return [];
  return invoke<AssetGovernanceRecord[]>("load_asset_governance", { logicalIds });
}

export async function selectAssetVariant(logicalId: string, sha256: string): Promise<AssetGovernanceRecord> {
  if (!isTauri()) throw new Error("版本选择需要在桌面应用中执行。");
  return invoke<AssetGovernanceRecord>("select_asset_variant", {
    logicalId,
    sha256,
    confirmation: `SELECT:${logicalId}:${sha256}`
  });
}

export async function confirmAssetLicense(logicalId: string, decision: string, evidence: string): Promise<AssetGovernanceRecord> {
  if (!isTauri()) throw new Error("许可确认需要在桌面应用中执行。");
  return invoke<AssetGovernanceRecord>("confirm_asset_license", {
    logicalId,
    decision,
    evidence,
    confirmation: `LICENSE:${logicalId}`
  });
}

export async function confirmAssetVerification(logicalId: string, sha256: string, evidence: string): Promise<AssetGovernanceRecord> {
  if (!isTauri()) throw new Error("验证登记需要在桌面应用中执行。");
  return invoke<AssetGovernanceRecord>("confirm_asset_verification", {
    logicalId,
    sha256,
    evidence,
    confirmation: `VERIFY:${logicalId}:${sha256}`
  });
}

export async function createOptimizationDraft(logicalId: string, sha256: string): Promise<AssetGovernanceRecord> {
  if (!isTauri()) throw new Error("优化草稿需要在桌面应用中创建。");
  return invoke<AssetGovernanceRecord>("create_optimization_draft", { logicalId, sha256 });
}

export async function applyOptimizationDraft(logicalId: string, draftId: string): Promise<AssetGovernanceRecord> {
  if (!isTauri()) throw new Error("优化草稿需要在桌面应用中应用。");
  return invoke<AssetGovernanceRecord>("apply_optimization_draft", {
    logicalId,
    draftId,
    confirmation: `APPLY_OPTIMIZATION:${logicalId}:${draftId}`
  });
}

/** Definition reads require the desktop's validated registry paths. */
export function supportsDefinitionReads(): boolean {
  return typeof window !== "undefined" && isTauri();
}

export async function readAssetDefinition(logicalId: string, sha256: string): Promise<import("./types").AssetDefinition> {
  if (!supportsDefinitionReads()) throw new Error("definition.desktopRequired");
  return invoke("read_asset_definition", { logicalId, sha256 });
}

export async function searchAssetDefinitions(query: string): Promise<import("./types").DefinitionSearchResult> {
  if (!supportsDefinitionReads()) throw new Error("definition.desktopRequired");
  return invoke("search_asset_definitions", { query });
}
