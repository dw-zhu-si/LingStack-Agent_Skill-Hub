#[cfg(feature = "public-release")]
use crate::control::local_asset_roots;
#[cfg(feature = "public-release")]
use crate::types::{AssetGroup, Location, Variant};
use crate::types::{InventoryEnvelope, InventorySource, RefreshResult, RefreshStep, Registry};
use chrono::{DateTime, Local};
#[cfg(feature = "public-release")]
use serde_json::json;
#[cfg(feature = "public-release")]
use sha2::{Digest, Sha256};
#[cfg(not(feature = "public-release"))]
use std::collections::VecDeque;
#[cfg(feature = "public-release")]
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
#[cfg(not(feature = "public-release"))]
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
#[cfg(not(feature = "public-release"))]
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(feature = "public-release"))]
use std::thread::{self, JoinHandle};
#[cfg(not(feature = "public-release"))]
use std::time::Duration;
use std::time::UNIX_EPOCH;
#[cfg(not(feature = "public-release"))]
use wait_timeout::ChildExt;
#[cfg(feature = "public-release")]
use walkdir::WalkDir;

#[cfg(not(feature = "public-release"))]
pub const MANAGEMENT_RELATIVE: &str = ".config/lingstack/management";
#[cfg(not(feature = "public-release"))]
pub const ICLOUD_MANAGEMENT_RELATIVE: &str = ".local/share/lingstack/management";
#[cfg(not(feature = "public-release"))]
pub const REGISTRY_FILE: &str = "registry.json";
#[cfg(not(feature = "public-release"))]
const APP_CACHE_DIR: &str = "app.lingzhan.agent-skill-hub/cache";
#[cfg(feature = "public-release")]
const APP_CACHE_DIR: &str = "app.lingzhan.agent-skill-hub/cache";
const CACHE_REGISTRY_FILE: &str = "registry.json";
#[cfg(not(feature = "public-release"))]
const CACHE_SOURCE_FILE: &str = "registry-source.json";
static REFRESH_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "public-release")]
const MAX_PORTABLE_DEFINITIONS: usize = 10_000;
#[cfg(feature = "public-release")]
const MAX_PORTABLE_DEFINITION_BYTES: u64 = 2 * 1024 * 1024;
#[cfg(feature = "public-release")]
const PUBLIC_CACHE_EPOCH: u64 = 2;

struct RefreshPermit;

impl RefreshPermit {
    fn acquire() -> Result<Self, String> {
        REFRESH_IN_PROGRESS
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| "已有资产刷新任务正在运行，请等待当前任务完成".to_string())
    }
}

impl Drop for RefreshPermit {
    fn drop(&mut self) {
        REFRESH_IN_PROGRESS.store(false, Ordering::Release);
    }
}

#[cfg(not(feature = "public-release"))]
pub fn management_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法定位当前用户主目录".to_string())?;
    let primary = home.join(MANAGEMENT_RELATIVE);
    let icloud = home.join(ICLOUD_MANAGEMENT_RELATIVE);
    if icloud.join(REGISTRY_FILE).exists() {
        return Ok(icloud);
    }
    Ok(primary)
}

#[cfg(feature = "public-release")]
pub fn management_dir() -> Result<PathBuf, String> {
    Ok(cache_dir()?.join("governance-records"))
}

#[cfg(not(feature = "public-release"))]
pub fn registry_path() -> Result<PathBuf, String> {
    Ok(management_dir()?.join(REGISTRY_FILE))
}

#[cfg(feature = "public-release")]
pub fn registry_path() -> Result<PathBuf, String> {
    Ok(cache_dir()?.join(CACHE_REGISTRY_FILE))
}

fn cache_dir() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|path| path.join(APP_CACHE_DIR))
        .ok_or_else(|| "无法定位应用缓存目录".to_string())
}

#[cfg(feature = "public-release")]
fn write_registry_atomic(path: &Path, registry: &Registry) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "资产库路径缺少父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建应用资产库：{error}"))?;
    let temporary = parent.join(".registry.json.tmp");
    let bytes = serde_json::to_vec_pretty(registry)
        .map_err(|error| format!("无法序列化应用资产库：{error}"))?;
    fs::write(&temporary, bytes).map_err(|error| format!("无法写入应用资产库：{error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("无法原子替换应用资产库：{error}"))
}

#[cfg(feature = "public-release")]
fn empty_public_registry() -> Registry {
    Registry {
        schema_version: 2,
        policy: json!({
            "distribution": "clean-public",
            "bundled_agent_skill_count": 0,
            "discovery": "manual-local-only",
            "public_cache_epoch": PUBLIC_CACHE_EPOCH,
            "truth_boundary": "文件发现不等于已验证或可分发"
        }),
        physical_entry_count: 0,
        logical_asset_count: 0,
        unique_hash_count: 0,
        groups: Vec::new(),
        updated_at: Local::now().to_rfc3339(),
    }
}

#[cfg(feature = "public-release")]
fn ensure_public_registry(path: &Path) -> Result<(), String> {
    if path.exists() {
        let cache_is_current = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value["policy"]["public_cache_epoch"].as_u64())
            == Some(PUBLIC_CACHE_EPOCH);
        if cache_is_current {
            return Ok(());
        }
    }
    write_registry_atomic(path, &empty_public_registry())
}

#[cfg(feature = "public-release")]
fn metadata_value(content: &str, keys: &[&str]) -> String {
    let lines: Vec<&str> = content.lines().take(120).collect();
    let frontmatter = lines.first().is_some_and(|line| line.trim() == "---");
    for (index, line) in lines.iter().enumerate().skip(usize::from(frontmatter)) {
        let trimmed = line.trim();
        if frontmatter && matches!(trimmed, "---" | "...") {
            break;
        }
        // Nested metadata and body examples must not override top-level fields.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        for key in keys {
            for separator in [':', '='] {
                if let Some(value) = trimmed
                    .strip_prefix(key)
                    .and_then(|rest| rest.trim_start().strip_prefix(separator))
                {
                    let value = value.trim();
                    if matches!(value, "|" | "|-" | "|+" | ">" | ">-" | ">+") {
                        let block = lines[index + 1..]
                            .iter()
                            .take_while(|line| {
                                line.trim().is_empty() || line.starts_with(char::is_whitespace)
                            })
                            .map(|line| line.trim())
                            .collect::<Vec<_>>()
                            .join(if value.starts_with('>') { " " } else { "\n" });
                        return block.trim().chars().take(1024).collect();
                    }
                    return value.trim_matches(['\'', '"']).chars().take(1024).collect();
                }
            }
        }
    }
    String::new()
}

#[cfg(feature = "public-release")]
fn portable_slug(kind: &str, name: &str) -> String {
    let mut slug = name
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let shortened = slug.trim_matches('-').chars().count() > 100;
    slug = slug.trim_matches('-').chars().take(100).collect();
    if slug.is_empty() {
        slug = format!("{:x}", Sha256::digest(name.as_bytes()))[..16].to_string();
    } else if shortened
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || c == '-')
    {
        // Ordinary ASCII names retain legacy case/whitespace/hyphen aliases.
        // Truncation, punctuation and Unicode require a lossless identity suffix.
        slug.push('-');
        slug.push_str(&format!("{:x}", Sha256::digest(name.as_bytes()))[..16]);
    }
    format!("local-{}-{slug}", kind.to_ascii_lowercase())
}

#[cfg(feature = "public-release")]
fn portable_definition_paths(kind: &str, root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    if !root.is_dir() {
        return Vec::new();
    }
    let is_skill = kind.eq_ignore_ascii_case("skill");
    WalkDir::new(root)
        .follow_links(false)
        .max_depth(9)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            if is_skill {
                return path.file_name().is_some_and(|name| name == "SKILL.md");
            }
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("md" | "toml" | "yaml" | "yml" | "json")
            )
        })
        .take(MAX_PORTABLE_DEFINITIONS)
        .collect()
}

#[cfg(feature = "public-release")]
fn portable_registry() -> Result<Registry, String> {
    portable_registry_from_roots(local_asset_roots()?)
}

#[cfg(feature = "public-release")]
fn portable_registry_from_roots(roots: Vec<(String, String, PathBuf)>) -> Result<Registry, String> {
    let mut groups = BTreeMap::<String, AssetGroup>::new();
    let mut unique_hashes = BTreeSet::new();
    let mut physical_entry_count = 0_u64;

    for (tool, kind, root) in roots {
        for path in portable_definition_paths(&kind, &root) {
            if physical_entry_count as usize >= MAX_PORTABLE_DEFINITIONS {
                break;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() == 0
                || metadata.len() > MAX_PORTABLE_DEFINITION_BYTES
            {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let content = String::from_utf8_lossy(&bytes);
            let fallback_name = if kind.eq_ignore_ascii_case("skill") {
                path.parent()
                    .and_then(Path::file_name)
                    .and_then(|value| value.to_str())
                    .unwrap_or("未命名 Skill")
            } else {
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("未命名 Agent")
            };
            let parsed_name = metadata_value(&content, &["name", "title", "display_name"]);
            let name = if parsed_name.is_empty() {
                fallback_name.to_string()
            } else {
                parsed_name
            };
            let logical_id = portable_slug(&kind, &name);
            let description =
                metadata_value(&content, &["description", "short_description", "summary"]);
            let license = metadata_value(&content, &["license", "licence"]);
            let sha256 = format!("{:x}", Sha256::digest(&bytes));
            unique_hashes.insert(sha256.clone());
            physical_entry_count += 1;

            let group = groups
                .entry(logical_id.clone())
                .or_insert_with(|| AssetGroup {
                    logical_id,
                    kind: kind.clone(),
                    name: name.clone(),
                    description,
                    lifecycle_state: "本机可见待验证".to_string(),
                    provenance: vec!["portable_local_discovery".to_string()],
                    license: if license.is_empty() {
                        "待确认".to_string()
                    } else {
                        license.clone()
                    },
                    ..Default::default()
                });
            if !group.tools.contains(&tool) {
                group.tools.push(tool.clone());
                group.tools.sort();
            }
            if group.license == "待确认" && !license.is_empty() {
                group.license = license;
            }
            let location = Location {
                path: path.display().to_string(),
                source_class: format!("{tool} 本机发现"),
                project: String::new(),
                project_root: String::new(),
            };
            if let Some(variant) = group
                .variants
                .iter_mut()
                .find(|variant| variant.sha256 == sha256)
            {
                if !variant
                    .locations
                    .iter()
                    .any(|existing| existing.path == location.path)
                {
                    variant.locations.push(location);
                }
            } else {
                group.variants.push(Variant {
                    sha256,
                    locations: vec![location],
                    states: vec!["本机可见待验证".to_string()],
                    snapshots: Vec::new(),
                });
            }
        }
    }

    let mut groups = groups.into_values().collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(Registry {
        schema_version: 2,
        policy: json!({
            "distribution": "clean-public",
            "bundled_agent_skill_count": 0,
            "discovery": "manual-local-only",
            "public_cache_epoch": PUBLIC_CACHE_EPOCH,
            "truth_boundary": "仅扫描当前设备显式工具入口；不执行、不上传、不自动验证"
        }),
        physical_entry_count,
        logical_asset_count: groups.len() as u64,
        unique_hash_count: unique_hashes.len() as u64,
        groups,
        updated_at: Local::now().to_rfc3339(),
    })
}

#[cfg(not(feature = "public-release"))]
fn copy_with_timeout(source: &Path, destination: &Path) -> Result<(), String> {
    let mut child = Command::new("/bin/cp")
        .arg(source)
        .arg(destination)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("无法启动事实源缓存复制：{error}"))?;
    match child.wait_timeout(Duration::from_secs(8)) {
        Ok(Some(status)) if status.success() => Ok(()),
        Ok(Some(status)) => Err(format!(
            "事实源缓存复制失败，退出码 {}",
            status.code().unwrap_or(-1)
        )),
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            Err("事实源缓存复制超过 8 秒，已停止本次复制".to_string())
        }
        Err(error) => Err(format!("等待事实源缓存复制失败：{error}")),
    }
}

#[cfg(not(feature = "public-release"))]
fn cached_inventory_path() -> Result<(PathBuf, InventorySource), String> {
    let source_path = registry_path()?;
    let source = inventory_source_for(&source_path)?;
    let directory = cache_dir()?;
    let cache = directory.join(CACHE_REGISTRY_FILE);
    let sidecar = directory.join(CACHE_SOURCE_FILE);
    let cached_source = fs::read(&sidecar)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<InventorySource>(&bytes).ok());
    if cache.exists()
        && cached_source
            .as_ref()
            .is_some_and(|value| value.source_fingerprint == source.source_fingerprint)
    {
        return Ok((cache, source));
    }

    fs::create_dir_all(&directory).map_err(|error| format!("无法创建注册表缓存目录：{error}"))?;
    let temporary = directory.join(".registry.json.tmp");
    let copy_result = copy_with_timeout(&source_path, &temporary).and_then(|_| {
        let after = inventory_source_for(&source_path)?;
        if after.source_fingerprint != source.source_fingerprint {
            return Err("事实源在缓存期间发生变化，请稍后重试".to_string());
        }
        fs::rename(&temporary, &cache).map_err(|error| format!("无法替换注册表缓存：{error}"))?;
        let sidecar_temporary = directory.join(".registry-source.json.tmp");
        let bytes = serde_json::to_vec_pretty(&source)
            .map_err(|error| format!("无法序列化缓存回执：{error}"))?;
        fs::write(&sidecar_temporary, bytes)
            .map_err(|error| format!("无法写入缓存回执：{error}"))?;
        fs::rename(sidecar_temporary, &sidecar)
            .map_err(|error| format!("无法替换缓存回执：{error}"))
    });
    if let Err(error) = copy_result {
        let _ = fs::remove_file(&temporary);
        if cache.exists() {
            let cached = inventory_source_for(&cache)?;
            return Ok((
                cache,
                InventorySource {
                    source_path: format!("本地缓存（事实源暂不可读：{error}）"),
                    ..cached
                },
            ));
        }
        return Err(error);
    }
    Ok((cache, source))
}

#[cfg(feature = "public-release")]
fn cached_inventory_path() -> Result<(PathBuf, InventorySource), String> {
    let path = registry_path()?;
    ensure_public_registry(&path)?;
    let source = inventory_source_for(&path)?;
    Ok((path, source))
}

fn inventory_source_for(path: &Path) -> Result<InventorySource, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("无法读取统一注册表元数据 {}：{error}", path.display()))?;
    let modified = metadata
        .modified()
        .map_err(|error| format!("无法读取统一注册表修改时间 {}：{error}", path.display()))?;
    let modified_at = DateTime::<Local>::from(modified).to_rfc3339();
    let modified_nanos = modified
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let source_size_bytes = metadata.len();

    Ok(InventorySource {
        source_path: path.display().to_string(),
        source_modified_at: modified_at,
        source_size_bytes,
        source_fingerprint: format!("{modified_nanos:x}-{source_size_bytes:x}"),
    })
}

fn read_inventory_snapshot(path: &Path) -> Result<(Vec<u8>, InventorySource), String> {
    for _ in 0..2 {
        let before = inventory_source_for(path)?;
        let bytes = fs::read(path)
            .map_err(|error| format!("无法读取统一注册表 {}：{error}", path.display()))?;
        let after = inventory_source_for(path)?;
        if before.source_fingerprint == after.source_fingerprint {
            return Ok((bytes, after));
        }
    }

    Err(format!(
        "统一注册表在读取期间持续变化，请稍后重试：{}",
        path.display()
    ))
}

pub fn read_registry() -> Result<Registry, String> {
    let (path, _) = cached_inventory_path()?;
    let (bytes, _) = read_inventory_snapshot(&path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("统一注册表 JSON 结构无效 {}：{error}", path.display()))
}

#[tauri::command]
pub fn inspect_inventory() -> Result<InventorySource, String> {
    inventory_source_for(&registry_path()?)
}

fn load_inventory_blocking() -> Result<InventoryEnvelope, String> {
    let (path, source) = cached_inventory_path()?;
    let (bytes, _) = read_inventory_snapshot(&path)?;
    let registry = serde_json::from_slice(&bytes)
        .map_err(|error| format!("统一注册表 JSON 结构无效 {}：{error}", path.display()))?;

    Ok(InventoryEnvelope {
        registry,
        source_path: source.source_path,
        source_modified_at: source.source_modified_at,
        source_size_bytes: source.source_size_bytes,
        source_fingerprint: source.source_fingerprint,
    })
}

#[tauri::command]
pub async fn load_inventory() -> Result<InventoryEnvelope, String> {
    tauri::async_runtime::spawn_blocking(load_inventory_blocking)
        .await
        .map_err(|error| format!("加载资产任务异常终止：{error}"))?
}

#[cfg(not(feature = "public-release"))]
fn spawn_tail_reader<R>(reader: R) -> JoinHandle<String>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::new();
        let mut tail = VecDeque::with_capacity(3);

        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buffer).trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    let line = line.chars().take(420).collect::<String>();
                    if tail.len() == 3 {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
                Err(_) => break,
            }
        }

        summarize_output(&tail.into_iter().collect::<Vec<_>>().join("\n"))
    })
}

#[cfg(not(feature = "public-release"))]
fn finish_tail_reader(reader: Option<JoinHandle<String>>) -> String {
    reader
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
}

#[cfg(not(feature = "public-release"))]
fn run_python_step(name: &str, script: &Path, args: &[&str], timeout: Duration) -> RefreshStep {
    if !script.exists() {
        return RefreshStep {
            name: name.to_string(),
            status: "failed".to_string(),
            summary: format!("脚本不存在：{}", script.display()),
        };
    }

    let mut child = match Command::new("python3")
        .arg(script)
        .args(args)
        .current_dir(script.parent().unwrap_or_else(|| Path::new("/")))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return RefreshStep {
                name: name.to_string(),
                status: "failed".to_string(),
                summary: format!("无法启动：{error}"),
            };
        }
    };

    let stdout_reader = child.stdout.take().map(spawn_tail_reader);
    let stderr_reader = child.stderr.take().map(spawn_tail_reader);

    match child.wait_timeout(timeout) {
        Ok(Some(status)) => {
            let stdout = finish_tail_reader(stdout_reader);
            let stderr = finish_tail_reader(stderr_reader);
            let summary = if !status.success() && !stderr.is_empty() {
                stderr
            } else {
                stdout
            };
            RefreshStep {
                name: name.to_string(),
                status: if status.success() {
                    "success".to_string()
                } else {
                    "failed".to_string()
                },
                summary: if summary.is_empty() {
                    format!("退出码 {}", status.code().unwrap_or(-1))
                } else {
                    summary
                },
            }
        }
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = finish_tail_reader(stdout_reader);
            let _ = finish_tail_reader(stderr_reader);
            RefreshStep {
                name: name.to_string(),
                status: "timeout".to_string(),
                summary: format!(
                    "超过 {} 秒，已精确停止本次刷新进程；未触碰其他任务",
                    timeout.as_secs()
                ),
            }
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = finish_tail_reader(stdout_reader);
            let _ = finish_tail_reader(stderr_reader);
            RefreshStep {
                name: name.to_string(),
                status: "failed".to_string(),
                summary: format!("等待刷新进程失败：{error}"),
            }
        }
    }
}

#[cfg(not(feature = "public-release"))]
fn summarize_output(output: &str) -> String {
    let lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    lines
        .into_iter()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" · ")
        .chars()
        .take(420)
        .collect()
}

#[cfg(feature = "public-release")]
fn refresh_inventory_blocking(mode: String) -> Result<RefreshResult, String> {
    if mode != "local" && mode != "full" {
        return Err("刷新模式只能是 local 或 full".to_string());
    }
    let _permit = RefreshPermit::acquire()?;
    let registry = portable_registry()?;
    let logical_asset_count = registry.logical_asset_count;
    let physical_entry_count = registry.physical_entry_count;
    let registry_updated_at = registry.updated_at.clone();
    write_registry_atomic(&registry_path()?, &registry)?;
    let mut steps = vec![RefreshStep {
        name: "扫描当前设备工具入口".to_string(),
        status: "success".to_string(),
        summary: format!(
            "发现 {physical_entry_count} 个物理定义，聚合为 {logical_asset_count} 个本机资产；未执行或上传内容"
        ),
    }];
    if mode == "full" {
        steps.push(RefreshStep {
            name: "发布洁净版项目边界".to_string(),
            status: "success".to_string(),
            summary: "发行包不内置私人项目索引；深度范围由统一控制中用户手动登记的自定义入口决定"
                .to_string(),
        });
    }
    Ok(RefreshResult {
        mode,
        status: "success".to_string(),
        steps,
        registry_updated_at: Some(registry_updated_at),
    })
}

#[cfg(not(feature = "public-release"))]
fn refresh_inventory_blocking(mode: String) -> Result<RefreshResult, String> {
    if mode != "local" && mode != "full" {
        return Err("刷新模式只能是 local 或 full".to_string());
    }
    let _permit = RefreshPermit::acquire()?;

    let scripts = management_dir()?.join("scripts");
    let mut definitions: Vec<(&str, PathBuf, Vec<&str>, Duration)> = vec![(
        "刷新本机 Agent / Skill 清单",
        scripts.join("刷新Agent与Skill清单.py"),
        vec![],
        Duration::from_secs(75),
    )];

    if mode == "full" {
        definitions.extend([
            (
                "刷新全项目资源地图",
                scripts.join("刷新全项目Agent与Skill清单.py"),
                vec!["--skip-codex-workspaces", "--project-timeout-seconds", "12"],
                Duration::from_secs(120),
            ),
            (
                "捕获脱敏归档快照",
                scripts.join("管理Agent与Skill归档快照.py"),
                vec!["capture", "--apply", "--skip-codex-workspaces"],
                Duration::from_secs(120),
            ),
        ]);
    }

    definitions.push((
        "重建统一资产注册表",
        scripts.join("构建Agent与Skill统一注册表.py"),
        vec!["--skip-codex-workspaces"],
        Duration::from_secs(90),
    ));

    let mut steps = Vec::new();
    for (name, script, args, timeout) in definitions {
        let step = run_python_step(name, &script, &args, timeout);
        let failed = step.status != "success";
        steps.push(step);
        if failed {
            break;
        }
    }

    let success = steps.iter().all(|step| step.status == "success");
    Ok(RefreshResult {
        mode,
        status: if success { "success" } else { "blocked" }.to_string(),
        steps,
        registry_updated_at: None,
    })
}

#[tauri::command]
pub async fn refresh_inventory(mode: String) -> Result<RefreshResult, String> {
    tauri::async_runtime::spawn_blocking(move || refresh_inventory_blocking(mode))
        .await
        .map_err(|error| format!("刷新资产任务异常终止：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "public-release"))]
    #[test]
    fn summary_keeps_only_the_last_three_lines() {
        let summary = summarize_output("a\nb\nc\nd\n");
        assert_eq!(summary, "b · c · d");
    }

    #[cfg(feature = "public-release")]
    #[test]
    fn metadata_reads_multiline_descriptions_without_body_examples() {
        let text = "---\nname: sample\ndescription: >-\n  First line\n  第二行\nlicense: MIT\n---\nname: ignored\n";
        assert_eq!(metadata_value(text, &["description"]), "First line 第二行");
        assert_eq!(metadata_value(text, &["license"]), "MIT");
        assert_eq!(
            metadata_value(
                "---\nname: sample\n---\ndescription: body",
                &["description"]
            ),
            ""
        );
        assert_eq!(
            metadata_value("---\nmetadata:\n  name: nested\nname: real\n---", &["name"]),
            "real"
        );
        assert_eq!(
            metadata_value("name = \"TOML Agent\"", &["name"]),
            "TOML Agent"
        );
    }

    #[cfg(feature = "public-release")]
    #[test]
    fn unicode_names_do_not_collapse_into_ascii_slugs() {
        assert_eq!(
            portable_slug("Agent", "API Tester"),
            "local-agent-api-tester"
        );
        assert_ne!(
            portable_slug("Agent", &format!("{}A", "a".repeat(100))),
            portable_slug("Agent", &format!("{}B", "a".repeat(100)))
        );
        assert_ne!(
            portable_slug("Agent", "API.Tester"),
            portable_slug("Agent", "API Tester")
        );
        assert_eq!(
            portable_slug("Agent", "API  Tester"),
            portable_slug("Agent", "API-Tester")
        );
        assert_ne!(
            portable_slug("Agent", "API 测试"),
            portable_slug("Agent", "API 审计")
        );
        assert_ne!(
            portable_slug("Agent", "API 测试"),
            portable_slug("Agent", "API")
        );
        assert_eq!(
            portable_slug("Agent", "API 测试"),
            portable_slug("Agent", "API 测试")
        );
    }

    #[cfg(feature = "public-release")]
    #[test]
    fn clean_public_registry_starts_without_bundled_assets() {
        let registry = empty_public_registry();
        assert_eq!(registry.logical_asset_count, 0);
        assert_eq!(registry.physical_entry_count, 0);
        assert!(registry.groups.is_empty());
        assert_eq!(registry.policy["bundled_agent_skill_count"], 0);
        assert_eq!(registry.policy["public_cache_epoch"], PUBLIC_CACHE_EPOCH);
    }

    #[cfg(feature = "public-release")]
    #[test]
    fn legacy_public_cache_is_reset_at_the_clean_release_boundary() {
        let root =
            std::env::temp_dir().join(format!("lingzhan-public-cache-{}", std::process::id()));
        let path = root.join("registry.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            &path,
            r#"{"schema_version":2,"policy":{"distribution":"clean-public"},"physical_entry_count":1,"logical_asset_count":1,"unique_hash_count":1,"groups":[{"name":"private legacy asset"}],"updated_at":"legacy"}"#,
        )
        .unwrap();

        ensure_public_registry(&path).unwrap();
        let registry: Registry = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(registry.groups.is_empty());
        assert_eq!(registry.logical_asset_count, 0);
        assert_eq!(registry.policy["public_cache_epoch"], PUBLIC_CACHE_EPOCH);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature = "public-release")]
    #[test]
    fn portable_skill_scan_only_accepts_skill_entrypoints() {
        let root =
            std::env::temp_dir().join(format!("lingzhan-public-scan-{}", std::process::id()));
        let skill = root.join("sample");
        fs::create_dir_all(skill.join("references")).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: sample\ndescription: safe\n---\n",
        )
        .unwrap();
        fs::write(skill.join("references/notes.md"), "not an entrypoint").unwrap();
        let paths = portable_definition_paths("Skill", &root);
        assert_eq!(paths, vec![skill.join("SKILL.md")]);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature = "public-release")]
    #[test]
    fn portable_refresh_builds_a_registry_from_explicit_roots() {
        let root =
            std::env::temp_dir().join(format!("lingzhan-public-refresh-{}", std::process::id()));
        let agents = root.join("agents");
        let skills = root.join("skills/sample");
        fs::create_dir_all(&agents).unwrap();
        fs::create_dir_all(&skills).unwrap();
        fs::write(
            agents.join("reviewer.md"),
            "---\nname: Reviewer\ndescription: Reviews local work\n---\n",
        )
        .unwrap();
        fs::write(
            skills.join("SKILL.md"),
            "---\nname: Sample Skill\ndescription: Runs a sample\n---\n",
        )
        .unwrap();
        let registry = portable_registry_from_roots(vec![
            ("Test Tool".to_string(), "Agent".to_string(), agents),
            (
                "Test Tool".to_string(),
                "Skill".to_string(),
                root.join("skills"),
            ),
        ])
        .unwrap();
        assert_eq!(registry.physical_entry_count, 2);
        assert_eq!(registry.logical_asset_count, 2);
        assert_eq!(registry.unique_hash_count, 2);
        assert!(registry
            .groups
            .iter()
            .all(|asset| asset.lifecycle_state == "本机可见待验证"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refresh_permit_rejects_duplicate_work_and_releases_after_drop() {
        let first = RefreshPermit::acquire().expect("first refresh should acquire permit");
        assert!(RefreshPermit::acquire().is_err());
        drop(first);
        assert!(RefreshPermit::acquire().is_ok());
    }
}
