use crate::definition::open_without_links;
use crate::registry::read_registry;
use crate::relations::resolve_agent_skills_from_registry;
use crate::types::{AssetGroup, ExportRelationship, ExportResult, Registry};
use chrono::Local;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_BUNDLE_BYTES: u64 = 500 * 1024 * 1024;

#[derive(Debug)]
struct CandidateFile {
    source: PathBuf,
    archive_path: String,
    size: u64,
}

#[derive(Debug, Serialize)]
struct BundleManifest<'a> {
    schema_version: u8,
    exported_at: String,
    export_mode: &'a str,
    export_policy: ExportPolicy,
    registry_updated_at: &'a str,
    asset_count: usize,
    assets: &'a [AssetGroup],
    relationships: &'a [ExportRelationship],
    warnings: &'a [String],
    skipped: &'a [String],
}

#[derive(Debug, Serialize)]
struct ExportPolicy {
    selected_or_explicitly_related_assets_only: bool,
    preserves_source_and_hash: bool,
    candidate_state_is_not_install_approval: bool,
    excludes_secrets_and_private_data: bool,
    max_file_bytes: u64,
    max_bundle_bytes: u64,
}

fn export_policy() -> ExportPolicy {
    ExportPolicy {
        selected_or_explicitly_related_assets_only: true,
        preserves_source_and_hash: true,
        candidate_state_is_not_install_approval: true,
        excludes_secrets_and_private_data: true,
        max_file_bytes: MAX_FILE_BYTES,
        max_bundle_bytes: MAX_BUNDLE_BYTES,
    }
}

#[derive(Debug)]
struct ExportPlan {
    groups: Vec<AssetGroup>,
    relationships: Vec<ExportRelationship>,
    warnings: Vec<String>,
}

fn is_agent(group: &AssetGroup) -> bool {
    group.kind.eq_ignore_ascii_case("agent")
}

fn is_skill(group: &AssetGroup) -> bool {
    group.kind.eq_ignore_ascii_case("skill")
}

fn select_groups(registry: &Registry, logical_ids: &[String]) -> Result<Vec<AssetGroup>, String> {
    let requested: HashSet<&str> = logical_ids.iter().map(String::as_str).collect();
    let selected: Vec<AssetGroup> = registry
        .groups
        .iter()
        .filter(|group| requested.contains(group.logical_id.as_str()))
        .cloned()
        .collect();
    if selected.len() != requested.len() {
        return Err(format!(
            "选择中有 {} 项已不在当前注册表；请刷新后重新选择",
            requested.len().saturating_sub(selected.len())
        ));
    }
    Ok(selected)
}

fn build_export_plan(
    registry: &Registry,
    logical_ids: &[String],
    export_mode: &str,
) -> Result<ExportPlan, String> {
    if !matches!(
        export_mode,
        "selection" | "agents_only" | "skills_only" | "agent_with_skills"
    ) {
        return Err(
            "导出范围只能是 selection、agents_only、skills_only 或 agent_with_skills".to_string(),
        );
    }
    let selected = select_groups(registry, logical_ids)?;
    let empty_plan = |message: &str| Err(message.to_string());

    match export_mode {
        "selection" => Ok(ExportPlan {
            groups: selected,
            relationships: Vec::new(),
            warnings: Vec::new(),
        }),
        "agents_only" => {
            let groups: Vec<_> = selected.into_iter().filter(is_agent).collect();
            if groups.is_empty() {
                return empty_plan("当前选择中没有可导出的 Agent");
            }
            Ok(ExportPlan {
                groups,
                relationships: Vec::new(),
                warnings: Vec::new(),
            })
        }
        "skills_only" => {
            let groups: Vec<_> = selected.into_iter().filter(is_skill).collect();
            if groups.is_empty() {
                return empty_plan("当前选择中没有可导出的 Skill");
            }
            Ok(ExportPlan {
                groups,
                relationships: Vec::new(),
                warnings: Vec::new(),
            })
        }
        "agent_with_skills" => {
            let agents: Vec<_> = selected.iter().filter(|group| is_agent(group)).collect();
            if agents.len() != 1 {
                return empty_plan("Agent 协作包必须且只能选择 1 个 Agent");
            }
            let agent = agents[0];
            let resolution = resolve_agent_skills_from_registry(registry, &agent.logical_id)?;
            let mut included_ids = BTreeSet::from([agent.logical_id.clone()]);
            let mut relationships = Vec::new();
            let explicit_skill_ids: HashSet<String> = resolution
                .skills
                .iter()
                .map(|skill| skill.logical_id.clone())
                .collect();

            for skill in resolution.skills {
                included_ids.insert(skill.logical_id.clone());
                relationships.push(ExportRelationship {
                    agent_logical_id: agent.logical_id.clone(),
                    skill_logical_id: skill.logical_id,
                    source: "explicit_agent_definition".to_string(),
                    evidence: skill.evidence,
                });
            }
            for skill in selected.iter().filter(|group| is_skill(group)) {
                included_ids.insert(skill.logical_id.clone());
                if !explicit_skill_ids.contains(&skill.logical_id) {
                    relationships.push(ExportRelationship {
                        agent_logical_id: agent.logical_id.clone(),
                        skill_logical_id: skill.logical_id.clone(),
                        source: "user_selected".to_string(),
                        evidence: Vec::new(),
                    });
                }
            }
            if relationships.is_empty() {
                return empty_plan(
                    "该 Agent 没有可解析的显式配合 Skill；请先手动选择至少一个 Skill",
                );
            }

            let groups = registry
                .groups
                .iter()
                .filter(|group| included_ids.contains(&group.logical_id))
                .cloned()
                .collect();
            let mut warnings = resolution.warnings;
            if !resolution.unresolved_references.is_empty() {
                warnings.push(format!(
                    "未自动加入 {} 个无法唯一匹配的 Skill 引用：{}",
                    resolution.unresolved_references.len(),
                    resolution.unresolved_references.join("、")
                ));
            }
            Ok(ExportPlan {
                groups,
                relationships,
                warnings,
            })
        }
        _ => unreachable!(),
    }
}

fn safe_slug(value: &str) -> String {
    let mut output = String::with_capacity(value.len().min(96));
    for ch in value.chars().take(96) {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            output.push(ch);
        } else if ch.is_whitespace() || ch == '/' || ch == '\\' {
            output.push('-');
        }
    }
    if output.is_empty() {
        "asset".to_string()
    } else {
        output
    }
}

fn is_sensitive(path: &Path) -> bool {
    for component in path.components() {
        if let Component::Normal(value) = component {
            let part = value.to_string_lossy().to_ascii_lowercase();
            if matches!(
                part.as_str(),
                ".git"
                    | "node_modules"
                    | "target"
                    | "__pycache__"
                    | ".venv"
                    | "venv"
                    | "cache"
                    | ".cache"
                    | "logs"
            ) {
                return true;
            }
        }
    }

    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    name == ".env"
        || name.starts_with(".env.")
        || matches!(
            name.as_str(),
            "id_rsa" | "id_ed25519" | "cookies.txt" | "credentials.json"
        )
        || [
            ".pem", ".p12", ".pfx", ".key", ".sqlite", ".sqlite3", ".db", ".cookie",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix))
        || name.contains("access_token")
        || name.contains("refresh_token")
        || name.contains("private_key")
        || name.contains("credential")
}

fn collect_from_location(
    location: &Path,
    asset_slug: &str,
    variant_index: usize,
    seen: &mut HashSet<PathBuf>,
    candidates: &mut Vec<CandidateFile>,
    skipped: &mut Vec<String>,
    total_bytes: &mut u64,
) {
    let canonical = match fs::canonicalize(location) {
        Ok(value) => value,
        Err(_) => {
            skipped.push(format!("不可访问：{}", location.display()));
            return;
        }
    };
    if !seen.insert(canonical.clone()) {
        return;
    }

    let (root, single_file) = if canonical.is_dir() {
        (canonical.clone(), None)
    } else if canonical
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("SKILL.md"))
    {
        (
            canonical
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| canonical.clone()),
            None,
        )
    } else {
        (
            canonical
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| canonical.clone()),
            Some(canonical.clone()),
        )
    };

    let paths: Vec<PathBuf> = if let Some(file) = single_file {
        vec![file]
    } else {
        WalkDir::new(&root)
            .follow_links(false)
            .max_depth(8)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file() && !entry.file_type().is_symlink())
            .map(|entry| entry.into_path())
            .collect()
    };

    for path in paths {
        if is_sensitive(&path) {
            skipped.push(format!("敏感或缓存内容已排除：{}", path.display()));
            continue;
        }
        let metadata = match fs::metadata(&path) {
            Ok(value) => value,
            Err(_) => {
                skipped.push(format!("无法读取元数据：{}", path.display()));
                continue;
            }
        };
        if metadata.len() > MAX_FILE_BYTES {
            skipped.push(format!(
                "单文件超过 {} MiB：{}",
                MAX_FILE_BYTES / 1024 / 1024,
                path.display()
            ));
            continue;
        }
        if total_bytes.saturating_add(metadata.len()) > MAX_BUNDLE_BYTES {
            skipped.push(format!("达到导出包容量上限，未加入：{}", path.display()));
            continue;
        }

        let relative = path.strip_prefix(&root).unwrap_or(&path);
        let relative_string = if relative.as_os_str().is_empty() {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        } else {
            relative.to_string_lossy().to_string()
        };
        let archive_path = format!(
            "assets/{}/variant-{}/{}",
            asset_slug,
            variant_index + 1,
            relative_string.replace('\\', "/")
        );
        *total_bytes += metadata.len();
        candidates.push(CandidateFile {
            source: path,
            archive_path,
            size: metadata.len(),
        });
    }
}

fn collect_candidate_files(groups: &[AssetGroup]) -> (Vec<CandidateFile>, Vec<String>) {
    let mut candidates = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = HashSet::new();
    let mut total_bytes = 0_u64;

    for group in groups {
        let slug = safe_slug(&group.logical_id);
        for (variant_index, variant) in group.variants.iter().enumerate() {
            for location in &variant.locations {
                collect_from_location(
                    Path::new(&location.path),
                    &slug,
                    variant_index,
                    &mut seen,
                    &mut candidates,
                    &mut skipped,
                    &mut total_bytes,
                );
            }
        }
    }
    (candidates, skipped)
}

fn write_json(
    destination: &Path,
    registry: &Registry,
    export_mode: &str,
    groups: &[AssetGroup],
    relationships: &[ExportRelationship],
    warnings: &[String],
) -> Result<(), String> {
    let output = serde_json::json!({
        "schema_version": 1,
        "exported_at": Local::now().to_rfc3339(),
        "source_registry_updated_at": registry.updated_at,
        "export_mode": export_mode,
        "export_policy": export_policy(),
        "asset_count": groups.len(),
        "assets": groups,
        "relationships": relationships,
        "warnings": warnings
    });
    let bytes = serde_json::to_vec_pretty(&output).map_err(|error| error.to_string())?;
    fs::write(destination, bytes)
        .map_err(|error| format!("无法写入 {}：{error}", destination.display()))
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn write_markdown(
    destination: &Path,
    registry: &Registry,
    export_mode: &str,
    groups: &[AssetGroup],
    relationships: &[ExportRelationship],
    warnings: &[String],
) -> Result<(), String> {
    let mut output = String::new();
    output.push_str("# Agent 与 Skill 导出清单\n\n");
    output.push_str(&format!(
        "- 导出时间：{}\n- 注册表更新时间：{}\n- 导出范围：{}\n- 资产数量：{}\n- 边界：此清单不证明安装、加载、验证、生产可用或发布状态。\n\n",
        Local::now().to_rfc3339(),
        registry.updated_at,
        export_mode,
        groups.len()
    ));
    output.push_str("| 类型 | 名称 | 状态 | 内容版本 | 工具 | 项目 | 许可证 |\n");
    output.push_str("|---|---|---|---:|---|---|---|\n");
    for group in groups {
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            markdown_cell(&group.kind),
            markdown_cell(&group.name),
            markdown_cell(&group.lifecycle_state),
            group.variants.len(),
            markdown_cell(&group.tools.join("、")),
            markdown_cell(&group.projects.join("、")),
            markdown_cell(&group.license)
        ));
    }
    if !relationships.is_empty() {
        output.push_str("\n## Agent 与 Skill 配合关系\n\n");
        output.push_str("| Agent | Skill | 关系来源 |\n");
        output.push_str("|---|---|---|\n");
        for relationship in relationships {
            output.push_str(&format!(
                "| {} | {} | {} |\n",
                markdown_cell(&relationship.agent_logical_id),
                markdown_cell(&relationship.skill_logical_id),
                markdown_cell(&relationship.source)
            ));
        }
    }
    if !warnings.is_empty() {
        output.push_str("\n## 警告\n\n");
        for warning in warnings {
            output.push_str(&format!("- {}\n", warning.replace('\n', " ")));
        }
    }
    fs::write(destination, output)
        .map_err(|error| format!("无法写入 {}：{error}", destination.display()))
}

// Read verified definition bytes once; the archive receives this exact snapshot.
fn definition_snapshots(groups: &[AssetGroup]) -> Result<HashMap<PathBuf, Vec<u8>>, String> {
    let mut snapshots = HashMap::new();
    let mut total = 0_u64;
    for group in groups {
        for variant in &group.variants {
            if variant.locations.is_empty() {
                return Err("导出版本没有定义位置".into());
            }
            for location in &variant.locations {
                let mut path = PathBuf::from(&location.path);
                if is_skill(group) && path.is_dir() {
                    path.push("SKILL.md");
                }
                // Open the uncanonicalized path first, rejecting links in every component.
                let file = open_without_links(&path)?;
                let path = fs::canonicalize(&path)
                    .map_err(|_| "导出定义不可访问，请刷新资产库".to_string())?;
                if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                    return Err("导出定义不是普通文件".into());
                }
                let mut bytes = Vec::new();
                file.take(MAX_FILE_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() as u64 > MAX_FILE_BYTES {
                    return Err("导出定义超过大小上限".into());
                }
                if format!("{:x}", Sha256::digest(&bytes)) != variant.sha256 {
                    return Err("导出定义已偏离登记 SHA-256，请刷新资产库后重新选择版本".into());
                }
                if let std::collections::hash_map::Entry::Vacant(entry) = snapshots.entry(path) {
                    total += bytes.len() as u64;
                    if total > MAX_BUNDLE_BYTES {
                        return Err("导出定义超过总容量上限".into());
                    }
                    entry.insert(bytes);
                }
            }
        }
    }
    Ok(snapshots)
}

struct BundleTemporary(PathBuf);
impl Drop for BundleTemporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_bundle(
    destination: &Path,
    registry: &Registry,
    export_mode: &str,
    groups: &[AssetGroup],
    relationships: &[ExportRelationship],
    warnings: &[String],
) -> Result<(usize, Vec<String>), String> {
    let snapshots = definition_snapshots(groups)?;
    let (candidates, skipped) = collect_candidate_files(groups);
    let included_paths: HashSet<_> = candidates
        .iter()
        .map(|candidate| &candidate.source)
        .collect();
    if snapshots.keys().any(|path| !included_paths.contains(path)) {
        return Err("导出策略排除了已登记定义，拒绝交付不完整版本包".into());
    }
    static NEXT_EXPORT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let temporary = destination.with_file_name(format!(
        ".lingstack-export-{}-{}.tmp",
        std::process::id(),
        NEXT_EXPORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("无法创建临时导出包：{error}"))?;
    let temporary = BundleTemporary(temporary);
    let mut zip = zip::ZipWriter::new(output);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let manifest = BundleManifest {
        schema_version: 1,
        exported_at: Local::now().to_rfc3339(),
        export_mode,
        export_policy: export_policy(),
        registry_updated_at: &registry.updated_at,
        asset_count: groups.len(),
        assets: groups,
        relationships,
        warnings,
        skipped: &skipped,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    zip.start_file("manifest.json", options)
        .map_err(|error| error.to_string())?;
    zip.write_all(&manifest_bytes)
        .map_err(|error| error.to_string())?;

    let mut written_bytes = 0_u64;
    for candidate in &candidates {
        zip.start_file(&candidate.archive_path, options)
            .map_err(|error| format!("无法写入 {}：{error}", candidate.archive_path))?;
        let owned;
        let bytes = if let Some(snapshot) = snapshots.get(&candidate.source) {
            snapshot.as_slice()
        } else {
            let input = open_without_links(&candidate.source)?;
            if !input.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("导出附件不是普通文件".into());
            }
            let mut buffer = Vec::new();
            input
                .take(MAX_FILE_BYTES + 1)
                .read_to_end(&mut buffer)
                .map_err(|e| e.to_string())?;
            owned = buffer;
            owned.as_slice()
        };
        written_bytes += bytes.len() as u64;
        if bytes.len() as u64 > MAX_FILE_BYTES || written_bytes > MAX_BUNDLE_BYTES {
            return Err("导出内容在读取时超过容量上限".into());
        }
        zip.write_all(bytes)
            .map_err(|error| format!("无法复制 {}：{error}", candidate.source.display()))?;
        debug_assert!(candidate.size <= MAX_FILE_BYTES);
    }
    let output = zip.finish().map_err(|error| error.to_string())?;
    output.sync_all().map_err(|error| error.to_string())?;
    drop(output);
    fs::rename(&temporary.0, destination).map_err(|error| format!("无法提交导出包：{error}"))?;
    Ok((candidates.len(), skipped))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut input = File::open(path)
        .map_err(|error| format!("无法读取导出产物 {}：{error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn export_assets_blocking(
    logical_ids: Vec<String>,
    format: String,
    destination: String,
    export_mode: String,
) -> Result<ExportResult, String> {
    if logical_ids.is_empty() {
        return Err("至少选择一项资产".to_string());
    }
    if !matches!(format.as_str(), "json" | "markdown" | "bundle") {
        return Err("导出格式只能是 json、markdown 或 bundle".to_string());
    }
    let destination = PathBuf::from(destination);
    if !destination.is_absolute() {
        return Err("导出路径必须是绝对路径".to_string());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建导出目录 {}：{error}", parent.display()))?;
    }

    let registry = read_registry()?;
    let plan = build_export_plan(&registry, &logical_ids, &export_mode)?;
    let agent_count = plan.groups.iter().filter(|group| is_agent(group)).count();
    let skill_count = plan.groups.iter().filter(|group| is_skill(group)).count();
    let mut warnings = plan.warnings.clone();
    let (file_count, skipped) = match format.as_str() {
        "json" => {
            write_json(
                &destination,
                &registry,
                &export_mode,
                &plan.groups,
                &plan.relationships,
                &plan.warnings,
            )?;
            (1, Vec::new())
        }
        "markdown" => {
            write_markdown(
                &destination,
                &registry,
                &export_mode,
                &plan.groups,
                &plan.relationships,
                &plan.warnings,
            )?;
            (1, Vec::new())
        }
        "bundle" => write_bundle(
            &destination,
            &registry,
            &export_mode,
            &plan.groups,
            &plan.relationships,
            &plan.warnings,
        )?,
        _ => unreachable!(),
    };
    let skipped_count = skipped.len();
    warnings.extend(skipped);
    let sha256 = sha256_file(&destination)?;
    Ok(ExportResult {
        destination: destination.display().to_string(),
        format,
        export_mode,
        asset_count: plan.groups.len(),
        agent_count,
        skill_count,
        relationship_count: plan.relationships.len(),
        file_count,
        warning_count: warnings.len(),
        skipped_count,
        sha256,
        warnings,
    })
}

#[tauri::command]
pub async fn export_assets(
    logical_ids: Vec<String>,
    format: String,
    destination: String,
    export_mode: String,
) -> Result<ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        export_assets_blocking(logical_ids, format, destination, export_mode)
    })
    .await
    .map_err(|error| format!("导出任务异常终止：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Location, Variant};

    #[test]
    fn sensitive_files_and_cache_directories_are_blocked() {
        assert!(is_sensitive(Path::new("/tmp/skill/.env")));
        assert!(is_sensitive(Path::new("/tmp/skill/node_modules/a.js")));
        assert!(is_sensitive(Path::new("/tmp/skill/private_key.pem")));
        assert!(is_sensitive(Path::new("/tmp/skill/session.sqlite")));
        assert!(!is_sensitive(Path::new("/tmp/skill/SKILL.md")));
        assert!(!is_sensitive(Path::new(
            "/tmp/skill/references/key-concepts.md"
        )));
    }

    #[test]
    fn markdown_escapes_table_breakers() {
        assert_eq!(markdown_cell("a|b\nc"), "a\\|b c");
    }

    #[test]
    fn slug_is_safe_for_zip_paths() {
        assert_eq!(safe_slug("Agent / 中文 name"), "Agent----name");
    }

    #[test]
    fn bundle_contains_skill_directory_but_excludes_env_files() {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "agent-skill-hub-export-test-{}",
            std::process::id()
        ));
        let skill_dir = root.join("sample-skill");
        fs::create_dir_all(skill_dir.join("references")).expect("create test skill");
        fs::write(skill_dir.join("SKILL.md"), "# Sample").expect("write skill");
        fs::write(skill_dir.join("references/guide.md"), "# Guide").expect("write guide");
        fs::write(skill_dir.join(".env"), "TOKEN=do-not-export").expect("write excluded fixture");

        let group = AssetGroup {
            logical_id: "sample-skill".to_string(),
            kind: "Skill".to_string(),
            name: "Sample Skill".to_string(),
            variants: vec![Variant {
                sha256: format!("{:x}", Sha256::digest(b"# Sample")),
                locations: vec![Location {
                    path: skill_dir.join("SKILL.md").display().to_string(),
                    ..Location::default()
                }],
                ..Variant::default()
            }],
            ..AssetGroup::default()
        };
        let registry = Registry {
            schema_version: 1,
            physical_entry_count: 1,
            logical_asset_count: 1,
            unique_hash_count: 1,
            groups: vec![group.clone()],
            updated_at: "test".to_string(),
            ..Registry::default()
        };
        let destination = root.join("bundle.zip");
        let relationship = ExportRelationship {
            agent_logical_id: "sample-agent".to_string(),
            skill_logical_id: "sample-skill".to_string(),
            source: "user_selected".to_string(),
            evidence: Vec::new(),
        };
        let (file_count, skipped) = write_bundle(
            &destination,
            &registry,
            "agent_with_skills",
            &[group],
            std::slice::from_ref(&relationship),
            &[],
        )
        .expect("write test bundle");
        assert_eq!(file_count, 2);
        assert!(skipped.iter().any(|item| item.contains(".env")));

        let file = File::open(&destination).expect("open bundle");
        let mut archive = zip::ZipArchive::new(file).expect("read bundle");
        let names: Vec<String> = (0..archive.len())
            .map(|index| {
                archive
                    .by_index(index)
                    .expect("zip entry")
                    .name()
                    .to_string()
            })
            .collect();
        assert!(names.iter().any(|name| name == "manifest.json"));
        assert!(names.iter().any(|name| name.ends_with("/SKILL.md")));
        assert!(names
            .iter()
            .any(|name| name.ends_with("/references/guide.md")));
        assert!(!names.iter().any(|name| name.ends_with("/.env")));
        let definition_name = names
            .iter()
            .find(|name| name.ends_with("/SKILL.md"))
            .unwrap();
        let mut definition_bytes = Vec::new();
        archive
            .by_name(definition_name)
            .unwrap()
            .read_to_end(&mut definition_bytes)
            .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&definition_bytes)),
            format!("{:x}", Sha256::digest(b"# Sample"))
        );
        let mut manifest = String::new();
        archive
            .by_name("manifest.json")
            .expect("manifest entry")
            .read_to_string(&mut manifest)
            .expect("read manifest");
        let manifest: serde_json::Value = serde_json::from_str(&manifest).expect("parse manifest");
        assert_eq!(manifest["export_mode"], "agent_with_skills");
        assert_eq!(manifest["relationships"][0]["source"], "user_selected");

        fs::remove_dir_all(&root).expect("remove test fixture");
    }

    #[test]
    fn drift_blocks_bundle_and_preserves_existing_destination() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("lingstack-export-drift-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("agent.md");
        fs::write(&source, "old").unwrap();
        let group = AssetGroup {
            kind: "Agent".into(),
            logical_id: "a".into(),
            variants: vec![Variant {
                sha256: format!("{:x}", Sha256::digest(b"old")),
                locations: vec![Location {
                    path: source.display().to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        fs::write(&source, "changed").unwrap();
        let destination = root.join("existing.zip");
        fs::write(&destination, "keep-existing").unwrap();
        assert!(write_bundle(
            &destination,
            &Registry::default(),
            "selection",
            &[group],
            &[],
            &[]
        )
        .unwrap_err()
        .contains("SHA-256"));
        assert_eq!(fs::read_to_string(&destination).unwrap(), "keep-existing");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_bundle_commit_removes_only_its_temporary_file() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("lingstack-export-cleanup-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("agent.md");
        fs::write(&source, "content").unwrap();
        let group = AssetGroup {
            kind: "Agent".into(),
            logical_id: "a".into(),
            variants: vec![Variant {
                sha256: format!("{:x}", Sha256::digest(b"content")),
                locations: vec![Location {
                    path: source.display().to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let destination = root.join("existing-directory");
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("keep"), "keep").unwrap();
        assert!(write_bundle(
            &destination,
            &Registry::default(),
            "selection",
            &[group],
            &[],
            &[]
        )
        .is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        assert_eq!(
            fs::read_to_string(destination.join("keep")).unwrap(),
            "keep"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn definition_export_rejects_links_and_non_regular_files() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("lingstack-export-links-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.md");
        fs::write(&source, "content").unwrap();
        let link = root.join("link.md");
        symlink(&source, &link).unwrap();
        let parent_link = root.join("parent");
        symlink(&root, &parent_link).unwrap();
        let group_for = |path: &Path| AssetGroup {
            kind: "Agent".into(),
            variants: vec![Variant {
                sha256: format!("{:x}", Sha256::digest(b"content")),
                locations: vec![Location {
                    path: path.display().to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        for path in [&link, &parent_link.join("source.md"), &root] {
            assert!(definition_snapshots(&[group_for(path)]).is_err());
        }
        // mkfifo creates only a task-owned local filesystem fixture, with no process or listener.
        #[cfg(target_os = "macos")]
        type FifoMode = u16;
        #[cfg(not(target_os = "macos"))]
        type FifoMode = u32;
        unsafe extern "C" {
            fn mkfifo(path: *const std::ffi::c_char, mode: FifoMode) -> i32;
        }
        use std::os::unix::ffi::OsStrExt;
        let fifo = root.join("pipe.md");
        let fifo_name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
        assert!(definition_snapshots(&[group_for(&fifo)]).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn kind_specific_plans_do_not_leak_other_assets() {
        let agent = AssetGroup {
            logical_id: "agent-one".to_string(),
            kind: "Agent".to_string(),
            name: "Agent One".to_string(),
            ..AssetGroup::default()
        };
        let skill = AssetGroup {
            logical_id: "skill-one".to_string(),
            kind: "Skill".to_string(),
            name: "Skill One".to_string(),
            ..AssetGroup::default()
        };
        let registry = Registry {
            schema_version: 1,
            physical_entry_count: 2,
            logical_asset_count: 2,
            unique_hash_count: 2,
            groups: vec![agent, skill],
            updated_at: "fixture".to_string(),
            ..Registry::default()
        };
        let ids = vec!["agent-one".to_string(), "skill-one".to_string()];

        let agent_plan =
            build_export_plan(&registry, &ids, "agents_only").expect("build agent plan");
        assert_eq!(agent_plan.groups.len(), 1);
        assert!(is_agent(&agent_plan.groups[0]));

        let skill_plan =
            build_export_plan(&registry, &ids, "skills_only").expect("build skill plan");
        assert_eq!(skill_plan.groups.len(), 1);
        assert!(is_skill(&skill_plan.groups[0]));
    }
}
