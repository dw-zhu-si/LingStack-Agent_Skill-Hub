use crate::registry::{management_dir, read_registry};
use crate::types::{
    AssetGovernanceRecord, AssetGroup, AssetLicenseReceipt, AssetVerificationReceipt,
    OptimizationDraftReceipt, Variant,
};
use crate::verification::audit_asset;
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[cfg(not(feature = "public-release"))]
const APP_DIR: &str = "app.lingzhan.agent-skill-hub";
#[cfg(feature = "public-release")]
const APP_DIR: &str = "app.lingzhan.agent-skill-hub";
const GOVERNANCE_FILE: &str = "asset-governance.json";
const MAX_DEFINITION_BYTES: u64 = 2 * 1024 * 1024;
const MAX_NOTE_CHARS: usize = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct GovernanceStore {
    #[serde(default)]
    records: Vec<AssetGovernanceRecord>,
}

fn data_dir() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|path| path.join(APP_DIR))
        .ok_or_else(|| "无法定位应用数据目录".to_string())
}

fn governance_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join(GOVERNANCE_FILE))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "目标路径缺少父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建目录：{error}"))?;
    let temporary = parent.join(format!(
        ".{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&temporary, bytes).map_err(|error| format!("无法写入临时文件：{error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("无法原子替换文件：{error}"))
}

fn load_store() -> Result<GovernanceStore, String> {
    let path = governance_path()?;
    if !path.exists() {
        return Ok(GovernanceStore::default());
    }
    let bytes = fs::read(&path).map_err(|error| format!("无法读取治理回执：{error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("治理回执结构无效：{error}"))
}

fn save_store(store: &GovernanceStore) -> Result<(), String> {
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|error| format!("无法序列化治理回执：{error}"))?;
    write_atomic(&governance_path()?, &bytes)
}

fn asset_by_id(logical_id: &str) -> Result<AssetGroup, String> {
    read_registry()?
        .groups
        .into_iter()
        .find(|asset| asset.logical_id == logical_id)
        .ok_or_else(|| "统一注册表中没有找到该资产".to_string())
}

fn definition_path(asset: &AssetGroup, location: &str) -> PathBuf {
    let path = PathBuf::from(location);
    if asset.kind.eq_ignore_ascii_case("skill") && path.is_dir() {
        path.join("SKILL.md")
    } else {
        path
    }
}

fn variant_for<'a>(asset: &'a AssetGroup, sha256: &str) -> Result<&'a Variant, String> {
    asset
        .variants
        .iter()
        .find(|variant| variant.sha256 == sha256)
        .ok_or_else(|| "所选哈希不属于该资产的当前版本".to_string())
}

fn structure_valid(kind: &str, bytes: &[u8]) -> bool {
    let content = String::from_utf8_lossy(bytes);
    if kind.eq_ignore_ascii_case("skill") {
        content.contains("name:") && content.contains("description:")
    } else {
        content.lines().any(|line| !line.trim().is_empty())
    }
}

fn validated_definition(asset: &AssetGroup, sha256: &str) -> Result<(PathBuf, Vec<u8>), String> {
    let variant = variant_for(asset, sha256)?;
    for location in &variant.locations {
        let path = definition_path(asset, &location.path);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() == 0
            || metadata.len() > MAX_DEFINITION_BYTES
        {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if actual == sha256 && structure_valid(&asset.kind, &bytes) {
            return Ok((path, bytes));
        }
    }
    Err("当前没有找到与所选 SHA-256 一致且结构可用的本机定义".to_string())
}

fn validate_note(note: &str, field: &str, minimum: usize) -> Result<String, String> {
    let clean = note.trim();
    let count = clean.chars().count();
    if count < minimum || count > MAX_NOTE_CHARS {
        return Err(format!("{field}需要 {minimum}–{MAX_NOTE_CHARS} 个字符"));
    }
    let lowered = clean.to_ascii_lowercase();
    if ["bearer ", "api_key=", "apikey=", "token=", "secret="]
        .iter()
        .any(|marker| lowered.contains(marker))
        || lowered.contains("sk-")
    {
        return Err(format!("{field}疑似包含密钥或令牌，已拒绝保存"));
    }
    Ok(clean.to_string())
}

fn license_pending(value: &str) -> bool {
    value.trim().is_empty() || value.contains('待') || value.contains("未知")
}

fn record_mut<'a>(
    store: &'a mut GovernanceStore,
    logical_id: &str,
) -> &'a mut AssetGovernanceRecord {
    if let Some(index) = store
        .records
        .iter()
        .position(|item| item.logical_id == logical_id)
    {
        return &mut store.records[index];
    }
    store.records.push(AssetGovernanceRecord {
        logical_id: logical_id.to_string(),
        ..Default::default()
    });
    store.records.last_mut().expect("record was just inserted")
}

fn slug(value: &str) -> String {
    let clean = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || character == '-'
                || character == '_'
                || ('\u{4e00}'..='\u{9fff}').contains(&character)
            {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    clean.trim_matches('-').chars().take(100).collect()
}

fn vault_root() -> Result<PathBuf, String> {
    management_dir()?
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法定位知识库根目录".to_string())
}

fn detail_path(asset: &AssetGroup) -> Result<PathBuf, String> {
    if !asset.detail_note.trim().is_empty() {
        let relative = PathBuf::from(&asset.detail_note);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Err("资产详细记录路径无效".to_string());
        }
        let mut path = vault_root()?.join(relative);
        path.set_extension("md");
        return Ok(path);
    }
    Ok(management_dir()?
        .join("详细记录")
        .join(if asset.kind.eq_ignore_ascii_case("agent") {
            "Agent"
        } else {
            "Skill"
        })
        .join(format!("{}.md", slug(&asset.name))))
}

fn append_text_atomic(path: &Path, addition: &str) -> Result<(), String> {
    let mut text = if path.exists() {
        fs::read_to_string(path).map_err(|error| format!("无法读取管理记录：{error}"))?
    } else {
        String::new()
    };
    if !text.ends_with('\n') && !text.is_empty() {
        text.push('\n');
    }
    text.push_str(addition);
    write_atomic(path, text.as_bytes())
}

fn ensure_detail_header(asset: &AssetGroup, path: &Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    let name = asset.name.replace(['\n', '\r', '"'], " ");
    let source = asset
        .variants
        .first()
        .and_then(|variant| variant.locations.first())
        .map(|location| location.path.as_str())
        .unwrap_or("待确认");
    let content = format!(
        "---\n类型: {}\n状态: 灵栈治理中\n稳定ID: {}\n创建时间: {}\n---\n\n# {}\n\n## 定位与边界\n\n- 一句话定位：{}\n- 主定义或已知来源：`{}`\n- 许可证：{}\n- 说明：本记录由用户在灵栈中显式启动治理操作后创建；不自动证明已安装、已加载、生产可用或可对外分发。\n\n## 灵栈治理回执\n",
        asset.kind,
        asset.logical_id,
        Local::now().format("%Y-%m-%d %H:%M:%S %z"),
        name,
        asset.description.if_empty("待补充"),
        source,
        asset.license.if_empty("待确认"),
    );
    write_atomic(path, content.as_bytes())
}

fn register_event(asset: &AssetGroup, title: &str, body: &str) -> Result<(), String> {
    let path = detail_path(asset)?;
    ensure_detail_header(asset, &path)?;
    let stamp = Local::now().format("%Y-%m-%d %H:%M:%S %z");
    append_text_atomic(&path, &format!("\n### {stamp} {title}\n\n{body}\n"))?;
    let change_log = management_dir()?.join("Agent与Skill变更记录.md");
    append_text_atomic(
        &change_log,
        &format!(
            "\n- {stamp} · `{}` · {} · {}。该回执不自动晋升 `ready` 或改写工具调用路径。\n",
            asset.logical_id,
            title,
            body.replace('\n', " ")
        ),
    )
}

trait EmptyFallback {
    fn if_empty(&self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(&self, fallback: &str) -> String {
        if self.trim().is_empty() {
            fallback.to_string()
        } else {
            self.clone()
        }
    }
}

#[tauri::command]
pub fn load_asset_governance(
    logical_ids: Vec<String>,
) -> Result<Vec<AssetGovernanceRecord>, String> {
    let mut store = load_store()?;
    let registry = read_registry()?;
    let mut changed = false;
    for record in &mut store.records {
        if record.pending_refresh
            && registry.groups.iter().any(|asset| {
                asset.logical_id == record.logical_id
                    && asset
                        .variants
                        .iter()
                        .any(|variant| variant.sha256 == record.selected_sha256)
            })
        {
            record.pending_refresh = false;
            changed = true;
        }
    }
    if changed {
        save_store(&store)?;
    }
    if logical_ids.is_empty() {
        return Ok(store.records);
    }
    let selected = logical_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    Ok(store
        .records
        .into_iter()
        .filter(|record| selected.contains(&record.logical_id))
        .collect())
}

#[tauri::command]
pub fn select_asset_variant(
    logical_id: String,
    sha256: String,
    confirmation: String,
) -> Result<AssetGovernanceRecord, String> {
    if confirmation != format!("SELECT:{logical_id}:{sha256}") {
        return Err("版本选择缺少精确确认".to_string());
    }
    let asset = asset_by_id(&logical_id)?;
    validated_definition(&asset, &sha256)?;
    let now = Local::now().to_rfc3339();
    let mut store = load_store()?;
    let result = {
        let record = record_mut(&mut store, &logical_id);
        record.selected_sha256 = sha256.clone();
        record.selected_at = now.clone();
        record.verification = None;
        record.pending_refresh = false;
        record.updated_at = now;
        record.clone()
    };
    save_store(&store)?;
    register_event(
        &asset,
        "手动选择固定版本",
        &format!("- 所选 SHA-256：`{sha256}`\n- 状态：已选择，待许可和验证证据。"),
    )?;
    Ok(result)
}

#[tauri::command]
pub fn confirm_asset_license(
    logical_id: String,
    decision: String,
    evidence: String,
    confirmation: String,
) -> Result<AssetGovernanceRecord, String> {
    if confirmation != format!("LICENSE:{logical_id}") {
        return Err("许可确认缺少精确确认".to_string());
    }
    let asset = asset_by_id(&logical_id)?;
    let decision = validate_note(&decision, "许可或使用边界", 2)?;
    let evidence = validate_note(&evidence, "许可证据", 4)?;
    let now = Local::now().to_rfc3339();
    let receipt = AssetLicenseReceipt {
        decision: decision.clone(),
        evidence,
        confirmed_at: now.clone(),
    };
    let mut store = load_store()?;
    let result = {
        let record = record_mut(&mut store, &logical_id);
        record.license = Some(receipt);
        record.updated_at = now;
        record.clone()
    };
    save_store(&store)?;
    register_event(
        &asset,
        "用户确认许可/使用边界",
        &format!("- 结论：{decision}\n- 证据已保存在本机治理回执，未写入密钥。"),
    )?;
    Ok(result)
}

#[tauri::command]
pub fn confirm_asset_verification(
    logical_id: String,
    sha256: String,
    evidence: String,
    confirmation: String,
) -> Result<AssetGovernanceRecord, String> {
    if confirmation != format!("VERIFY:{logical_id}:{sha256}") {
        return Err("验证登记缺少精确确认".to_string());
    }
    let asset = asset_by_id(&logical_id)?;
    validated_definition(&asset, &sha256)?;
    let audit = audit_asset(&asset, true);
    if audit.status == "blocked" {
        return Err("深度静态检查仍为阻塞，不能登记验证回执".to_string());
    }
    let evidence = validate_note(&evidence, "验证证据", 8)?;
    let now = Local::now().to_rfc3339();
    let receipt = AssetVerificationReceipt {
        sha256: sha256.clone(),
        evidence,
        truth_level: "local_static_plus_user_evidence".to_string(),
        status: "verified_local".to_string(),
        verified_at: now.clone(),
    };
    let mut store = load_store()?;
    let result = {
        let record = record_mut(&mut store, &logical_id);
        if asset.variants.len() > 1 && record.selected_sha256 != sha256 {
            return Err("多版本资产必须先手动选择同一 SHA-256".to_string());
        }
        if record.license.is_none() && license_pending(&asset.license) {
            return Err("请先确认许可证或内部使用边界".to_string());
        }
        record.selected_sha256 = sha256.clone();
        if record.selected_at.is_empty() {
            record.selected_at = now.clone();
        }
        record.verification = Some(receipt);
        record.pending_refresh = false;
        record.updated_at = now;
        record.clone()
    };
    save_store(&store)?;
    register_event(
        &asset,
        "登记本机验证回执",
        &format!("- SHA-256：`{sha256}`\n- 真值级别：深度静态验真 + 用户证据确认。\n- 边界：不自动晋升为跨项目 `ready`、生产可用或已发布。"),
    )?;
    Ok(result)
}

#[tauri::command]
pub fn create_optimization_draft(
    logical_id: String,
    sha256: String,
) -> Result<AssetGovernanceRecord, String> {
    let asset = asset_by_id(&logical_id)?;
    let (source, bytes) = validated_definition(&asset, &sha256)?;
    let audit = audit_asset(&asset, true);
    let draft_id = format!(
        "{}-{}",
        Local::now().format("%Y%m%d%H%M%S"),
        &sha256[..sha256.len().min(8)]
    );
    let directory = data_dir()?
        .join("optimization-drafts")
        .join(slug(&logical_id))
        .join(&draft_id);
    let working = directory
        .join("working")
        .join(source.file_name().unwrap_or_default());
    write_atomic(&working, &bytes)?;
    let plan = directory.join("OPTIMIZATION_PLAN.md");
    let suggestions = if audit.suggestions.is_empty() {
        "- 当前静态合同未发现明显缺口；优化时应聚焦真实使用失败或可测目标。".to_string()
    } else {
        audit
            .suggestions
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let plan_text = format!(
        "# {} 优化草稿\n\n- 稳定 ID：`{}`\n- 原定义：`{}`\n- 优化前 SHA-256：`{}`\n- 当前深检得分：{}\n\n## 建议\n\n{}\n\n## 操作边界\n\n只编辑 `working/` 内的工作副本。灵栈只有在用户显式点击“应用草稿”、源哈希未漂移、结构仍可用且许可边界已确认时，才会备份并替换原定义。\n",
        asset.name,
        asset.logical_id,
        source.display(),
        sha256,
        audit.score,
        suggestions
    );
    write_atomic(&plan, plan_text.as_bytes())?;
    let now = Local::now().to_rfc3339();
    let receipt = OptimizationDraftReceipt {
        id: draft_id,
        source_path: source.display().to_string(),
        source_sha256: sha256.clone(),
        working_file: working.display().to_string(),
        plan_path: plan.display().to_string(),
        status: "draft".to_string(),
        created_at: now.clone(),
        applied_at: String::new(),
        output_sha256: String::new(),
        backup_path: String::new(),
    };
    let mut store = load_store()?;
    let result = {
        let record = record_mut(&mut store, &logical_id);
        record.drafts.insert(0, receipt);
        record.drafts.truncate(20);
        record.updated_at = now;
        record.clone()
    };
    save_store(&store)?;
    register_event(
        &asset,
        "创建优化草稿",
        &format!(
            "- 基线 SHA-256：`{sha256}`\n- 草稿路径：`{}`\n- 状态：仅工作副本，未修改原定义。",
            working.display()
        ),
    )?;
    Ok(result)
}

fn apply_working_copy(
    kind: &str,
    source: &Path,
    working: &Path,
    expected_sha: &str,
    backup: &Path,
) -> Result<(String, String), String> {
    let metadata =
        fs::symlink_metadata(source).map_err(|error| format!("无法读取原定义：{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("原定义不是可安全替换的普通文件".to_string());
    }
    let source_bytes = fs::read(source).map_err(|error| format!("无法读取原定义：{error}"))?;
    let current_sha = format!("{:x}", Sha256::digest(&source_bytes));
    if current_sha != expected_sha {
        return Err("原定义在草稿创建后已变化，拒绝覆盖后续修改".to_string());
    }
    let working_bytes = fs::read(working).map_err(|error| format!("无法读取优化草稿：{error}"))?;
    if working_bytes.is_empty()
        || working_bytes.len() as u64 > MAX_DEFINITION_BYTES
        || !structure_valid(kind, &working_bytes)
    {
        return Err("优化草稿未通过最小结构检查".to_string());
    }
    let output_sha = format!("{:x}", Sha256::digest(&working_bytes));
    if output_sha == current_sha {
        return Err("优化草稿与原定义内容相同，没有可应用的变更".to_string());
    }
    write_atomic(backup, &source_bytes)?;
    let temporary = source.with_extension(format!(
        "{}.lingzhan-tmp",
        source.extension().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&temporary, &working_bytes)
        .map_err(|error| format!("无法写入临时优化定义：{error}"))?;
    fs::set_permissions(&temporary, metadata.permissions())
        .map_err(|error| format!("无法保留原定义权限：{error}"))?;
    fs::rename(&temporary, source).map_err(|error| format!("无法原子替换原定义：{error}"))?;
    Ok((current_sha, output_sha))
}

#[tauri::command]
pub fn apply_optimization_draft(
    logical_id: String,
    draft_id: String,
    confirmation: String,
) -> Result<AssetGovernanceRecord, String> {
    if confirmation != format!("APPLY_OPTIMIZATION:{logical_id}:{draft_id}") {
        return Err("应用优化草稿缺少精确确认".to_string());
    }
    let asset = asset_by_id(&logical_id)?;
    let mut store = load_store()?;
    let record_index = store
        .records
        .iter()
        .position(|record| record.logical_id == logical_id)
        .ok_or_else(|| "没有找到该资产的治理回执".to_string())?;
    if store.records[record_index].license.is_none() && license_pending(&asset.license) {
        return Err("应用优化前必须先确认许可或内部使用边界".to_string());
    }
    let draft_index = store.records[record_index]
        .drafts
        .iter()
        .position(|draft| draft.id == draft_id)
        .ok_or_else(|| "没有找到优化草稿".to_string())?;
    let draft = store.records[record_index].drafts[draft_index].clone();
    if draft.status != "draft" {
        return Err("该优化草稿已应用或不可再应用".to_string());
    }
    let source = PathBuf::from(&draft.source_path);
    let working = PathBuf::from(&draft.working_file);
    let draft_root = working
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "优化草稿路径结构无效".to_string())?;
    let backup = draft_root
        .join("backup")
        .join(source.file_name().unwrap_or_default());
    let (current_sha, output_sha) = apply_working_copy(
        &asset.kind,
        &source,
        &working,
        &draft.source_sha256,
        &backup,
    )?;
    let now = Local::now().to_rfc3339();
    {
        let record = &mut store.records[record_index];
        let draft = &mut record.drafts[draft_index];
        draft.status = "applied".to_string();
        draft.applied_at = now.clone();
        draft.output_sha256 = output_sha.clone();
        draft.backup_path = backup.display().to_string();
        record.selected_sha256 = output_sha.clone();
        record.verification = None;
        record.pending_refresh = true;
        record.updated_at = now;
    }
    save_store(&store)?;
    register_event(
        &asset,
        "应用优化草稿",
        &format!("- 优化前 SHA-256：`{current_sha}`\n- 优化后 SHA-256：`{output_sha}`\n- 回滚备份：`{}`\n- 状态：已修改所选源定义，待刷新清单、快照和注册表后重新验证。", backup.display()),
    )?;
    Ok(store.records[record_index].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_validation_rejects_likely_secrets() {
        assert!(validate_note("Bearer abcdefgh", "evidence", 4).is_err());
        assert!(validate_note("本机测试通过", "evidence", 4).is_ok());
    }

    #[test]
    fn skill_structure_requires_name_and_description() {
        assert!(structure_valid("Skill", b"name: demo\ndescription: test"));
        assert!(!structure_valid("Skill", b"# demo"));
        assert!(structure_valid("Agent", b"# Agent\nDoes work"));
    }

    #[test]
    fn working_copy_requires_unchanged_source_and_creates_backup() {
        let root = std::env::temp_dir().join(format!("lingzhan-governance-{}", std::process::id()));
        let source = root.join("source.md");
        let working = root.join("working.md");
        let backup = root.join("backup/source.md");
        fs::create_dir_all(&root).unwrap();
        fs::write(&source, "# Agent\nOriginal").unwrap();
        fs::write(&working, "# Agent\nOptimized").unwrap();
        let expected = format!("{:x}", Sha256::digest(fs::read(&source).unwrap()));
        let (_, output) =
            apply_working_copy("Agent", &source, &working, &expected, &backup).unwrap();
        assert_eq!(fs::read_to_string(&source).unwrap(), "# Agent\nOptimized");
        assert_eq!(fs::read_to_string(&backup).unwrap(), "# Agent\nOriginal");
        assert_eq!(
            output,
            format!("{:x}", Sha256::digest(b"# Agent\nOptimized"))
        );
        assert!(apply_working_copy("Agent", &source, &working, &expected, &backup).is_err());
        let _ = fs::remove_dir_all(root);
    }
}
