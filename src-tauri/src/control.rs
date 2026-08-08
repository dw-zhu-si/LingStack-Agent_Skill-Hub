use crate::types::{
    BindingReceipt, ControlCenterState, CustomToolBinding, ModelOption, ModelProfile,
    ModelTestResult, ToolBindingPreview,
};
use chrono::Local;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(not(feature = "app-store"))]
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
#[cfg(not(feature = "app-store"))]
use walkdir::WalkDir;

#[cfg(not(feature = "app-store"))]
const APP_DIR: &str = "app.lingzhan.agent-skill-hub";
#[cfg(feature = "app-store")]
const APP_DIR: &str = "app.lingzhan.lingstack.store";
const CONFIG_FILE: &str = "control-center.json";
const RECEIPTS_FILE: &str = "binding-receipts.json";
#[cfg(not(feature = "app-store"))]
const KEYCHAIN_SERVICE: &str = "app.lingzhan.agent-skill-hub.model-credentials";
#[cfg(feature = "app-store")]
const KEYCHAIN_SERVICE: &str = "app.lingzhan.lingstack.store.model-credentials";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ControlConfig {
    #[serde(default)]
    profiles: Vec<ModelProfile>,
    #[serde(default)]
    custom_bindings: Vec<CustomToolBinding>,
}

#[derive(Clone)]
struct BindingDefinition {
    id: String,
    tool: String,
    kind: String,
    source: PathBuf,
    detected: bool,
    custom: bool,
}

fn control_dir() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|path| path.join(APP_DIR))
        .ok_or_else(|| "无法定位应用数据目录".to_string())
}

fn unified_root() -> Result<PathBuf, String> {
    Ok(control_dir()?.join("unified"))
}

fn default_profiles() -> Vec<ModelProfile> {
    vec![ModelProfile {
        id: "ollama-local".to_string(),
        name: "Ollama 本机模型".to_string(),
        provider: "ollama".to_string(),
        endpoint: "http://127.0.0.1:11434".to_string(),
        model: "qwen3:8b".to_string(),
        models: vec!["qwen3:8b".to_string()],
        api_key_env: String::new(),
        enabled: true,
        credential_stored: false,
    }]
}

#[cfg(all(target_os = "macos", not(feature = "app-store")))]
fn keychain_secret(id: &str) -> Option<String> {
    security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, id)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

#[cfg(all(not(target_os = "macos"), not(feature = "app-store")))]
fn keychain_secret(_id: &str) -> Option<String> {
    None
}

#[cfg(all(target_os = "macos", not(feature = "app-store")))]
fn store_keychain_secret(id: &str, secret: &str) -> Result<(), String> {
    security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, id, secret.as_bytes())
        .map_err(|error| format!("无法写入 macOS 钥匙串：{error}"))
}

#[cfg(all(not(target_os = "macos"), not(feature = "app-store")))]
fn store_keychain_secret(_id: &str, _secret: &str) -> Result<(), String> {
    Err("当前平台暂不支持安全保存模型密钥".to_string())
}

#[cfg(target_os = "macos")]
fn delete_keychain_secret(id: &str) {
    let _ = security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, id);
}

#[cfg(not(target_os = "macos"))]
fn delete_keychain_secret(_id: &str) {}

fn read_config() -> Result<ControlConfig, String> {
    let path = control_dir()?.join(CONFIG_FILE);
    if !path.exists() {
        return Ok(ControlConfig {
            profiles: default_profiles(),
            custom_bindings: Vec::new(),
        });
    }
    let bytes = fs::read(&path).map_err(|error| format!("无法读取模型配置：{error}"))?;
    let mut config: ControlConfig =
        serde_json::from_slice(&bytes).map_err(|error| format!("模型配置结构无效：{error}"))?;
    #[cfg(feature = "app-store")]
    {
        let removed_ids = config
            .profiles
            .iter()
            .filter(|profile| !app_store_profile_allowed(profile))
            .map(|profile| profile.id.clone())
            .collect::<Vec<_>>();
        let mut changed = !removed_ids.is_empty();
        for id in removed_ids {
            delete_keychain_secret(&id);
        }
        config.profiles.retain(app_store_profile_allowed);
        if config.profiles.is_empty() {
            config.profiles = default_profiles();
            changed = true;
        }
        for profile in &mut config.profiles {
            normalize_profile_models(profile);
            if !profile.api_key_env.is_empty() || profile.credential_stored {
                changed = true;
            }
            profile.api_key_env.clear();
            profile.credential_stored = false;
            delete_keychain_secret(&profile.id);
        }
        if changed {
            write_json_atomic(&path, &config)?;
        }
    }
    #[cfg(not(feature = "app-store"))]
    for profile in &mut config.profiles {
        normalize_profile_models(profile);
        profile.credential_stored = keychain_secret(&profile.id).is_some();
    }
    Ok(config)
}

fn normalize_profile_models(profile: &mut ModelProfile) {
    profile.model = profile.model.trim().to_string();
    profile.models = profile
        .models
        .iter()
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .collect();
    if profile.models.is_empty() && !profile.model.is_empty() {
        profile.models.push(profile.model.clone());
    }
    profile.models.sort();
    profile.models.dedup();
    if profile.model.is_empty() {
        profile.model = profile.models.first().cloned().unwrap_or_default();
    } else if !profile.models.contains(&profile.model) {
        profile.models.push(profile.model.clone());
        profile.models.sort();
    }
}

/// 只清理端口中的分组逗号，避免把 `11,435` 交给 URL 解析器。
/// 其他位置的逗号保留，后续由严格 URL 校验拒绝。
fn normalize_endpoint_input(raw: &str) -> String {
    let value = raw.trim().replace('，', ",");
    let Some(scheme_end) = value.find("://") else {
        return value;
    };
    let authority_start = scheme_end + 3;
    let authority_end = value[authority_start..]
        .find('/')
        .map(|offset| authority_start + offset)
        .unwrap_or(value.len());
    let authority = &value[authority_start..authority_end];
    let Some(port_separator) = authority.rfind(':') else {
        return value;
    };
    let port = &authority[port_separator + 1..];
    if port.contains(',')
        && port
            .chars()
            .all(|character| character.is_ascii_digit() || character == ',')
    {
        let cleaned = port.replace(',', "");
        return format!(
            "{}{}{}",
            &value[..authority_start + port_separator + 1],
            cleaned,
            &value[authority_end..]
        );
    }
    value
}

#[cfg(feature = "app-store")]
fn app_store_profile_allowed(profile: &ModelProfile) -> bool {
    if profile.provider != "ollama" || !profile.api_key_env.is_empty() {
        return false;
    }
    Url::parse(profile.endpoint.trim())
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        })
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "配置路径缺少父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建配置目录：{error}"))?;
    let temporary = parent.join(format!(
        ".{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("无法序列化配置：{error}"))?;
    fs::write(&temporary, bytes).map_err(|error| format!("无法写入临时配置：{error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("无法原子替换配置：{error}"))
}

fn validate_profile(profile: &ModelProfile) -> Result<(), String> {
    if profile.id.is_empty()
        || !profile
            .id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("配置 ID 只能包含字母、数字、连字符和下划线".to_string());
    }
    if profile.name.trim().is_empty() {
        return Err("配置名称不能为空".to_string());
    }
    #[cfg(feature = "app-store")]
    if profile.provider != "ollama" {
        return Err("Mac App Store 版仅支持本机 Ollama".to_string());
    }
    #[cfg(not(feature = "app-store"))]
    if profile.provider != "ollama" && profile.provider != "openai_compatible" {
        return Err("当前仅支持 Ollama 与 OpenAI 兼容接口".to_string());
    }
    if profile.models.len() > 500
        || profile
            .models
            .iter()
            .any(|model| model.is_empty() || model.len() > 512)
    {
        return Err("单个接入最多管理 500 个模型，模型标识不能超过 512 字节".to_string());
    }
    let url =
        Url::parse(profile.endpoint.trim()).map_err(|_| "接口地址不是有效 URL".to_string())?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err("接口地址只允许 http 或 https".to_string());
    }
    if url.host_str().is_none() || url.query().is_some() || url.fragment().is_some() {
        return Err("接口地址必须包含主机，且不能含查询参数或片段".to_string());
    }
    #[cfg(feature = "app-store")]
    if !app_store_profile_allowed(profile) {
        return Err("Mac App Store 版的模型地址只能使用 localhost 或回环 IP".to_string());
    }
    if !profile.api_key_env.is_empty() {
        let mut chars = profile.api_key_env.chars();
        let first = chars.next().unwrap_or_default();
        if !(first.is_ascii_uppercase() || first == '_')
            || !chars.all(|character| {
                character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
            })
        {
            return Err(
                "密钥环境变量名只能使用大写字母、数字和下划线；真实密钥请填入钥匙串字段"
                    .to_string(),
            );
        }
    }
    Ok(())
}

#[cfg(not(feature = "app-store"))]
fn tool_detected(tool: &str, home: &Path) -> bool {
    match tool {
        "Codex" => home.join(".codex").exists(),
        "Claude" => home.join(".claude").exists(),
        "Trae" | "Trae CN" => {
            Path::new("/Applications/Trae.app").exists()
                || Path::new("/Applications/Trae CN.app").exists()
                || home.join("Library/Application Support/Trae").exists()
                || home.join("Library/Application Support/Trae CN").exists()
                || home.join(".trae").exists()
                || home.join(".trae-cn").exists()
        }
        "TRAE Work" => {
            Path::new("/Applications/TRAE SOLO CN.app").exists()
                || home
                    .join("Library/Application Support/TRAE SOLO CN")
                    .exists()
        }
        "Cursor" => home.join(".cursor").exists(),
        "GitHub Copilot" => home.join(".copilot").exists(),
        "Gemini CLI" => home.join(".gemini").exists(),
        "OpenCode" => home.join(".config/opencode").exists(),
        "Cline" => home.join(".cline").exists(),
        "Roo Code" => home.join(".roo").exists(),
        "Windsurf" => home.join(".codeium/windsurf").exists(),
        "Continue" => home.join(".continue").exists(),
        "Kiro" => home.join(".kiro").exists(),
        _ => false,
    }
}

#[cfg(not(feature = "app-store"))]
fn binding_definitions() -> Result<Vec<BindingDefinition>, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法定位当前用户主目录".to_string())?;
    let trae_root = home.join(".trae");
    let trae_cn_root = home.join(".trae-cn");
    let trae_work_root = home.join("Library/Application Support/TRAE SOLO CN/User");
    let candidates = vec![
        ("codex", "Codex", home.join(".codex"), "agents", "skills"),
        ("claude", "Claude", home.join(".claude"), "agents", "skills"),
        ("trae", "Trae", trae_root, "agents", "skills"),
        ("trae-cn", "Trae CN", trae_cn_root, "agents", "skills"),
        ("trae-work", "TRAE Work", trae_work_root, "agents", "skills"),
        ("cursor", "Cursor", home.join(".cursor"), "agents", "skills"),
        (
            "copilot",
            "GitHub Copilot",
            home.join(".copilot"),
            "agents",
            "skills",
        ),
        (
            "gemini",
            "Gemini CLI",
            home.join(".gemini"),
            "agents",
            "skills",
        ),
        (
            "opencode",
            "OpenCode",
            home.join(".config/opencode"),
            "agent",
            "skill",
        ),
        ("cline", "Cline", home.join(".cline"), "agents", "skills"),
        ("roo", "Roo Code", home.join(".roo"), "agents", "skills"),
        (
            "windsurf",
            "Windsurf",
            home.join(".codeium/windsurf"),
            "agents",
            "skills",
        ),
        (
            "continue",
            "Continue",
            home.join(".continue"),
            "agents",
            "skills",
        ),
        ("kiro", "Kiro", home.join(".kiro"), "agents", "skills"),
    ];
    let mut definitions = Vec::new();
    for (prefix, tool, root, agents, skills) in candidates {
        let detected = tool_detected(tool, &home);
        definitions.push(BindingDefinition {
            id: format!("{prefix}-agents"),
            tool: tool.to_string(),
            kind: "Agent".to_string(),
            source: root.join(agents),
            detected,
            custom: false,
        });
        definitions.push(BindingDefinition {
            id: format!("{prefix}-skills"),
            tool: tool.to_string(),
            kind: "Skill".to_string(),
            source: root.join(skills),
            detected,
            custom: false,
        });
    }
    for binding in read_config()?.custom_bindings {
        definitions.push(BindingDefinition {
            id: binding.id,
            tool: binding.tool,
            kind: binding.kind,
            detected: Path::new(&binding.source_path)
                .parent()
                .map(Path::exists)
                .unwrap_or(false),
            source: PathBuf::from(binding.source_path),
            custom: true,
        });
    }
    Ok(definitions)
}

#[cfg(feature = "app-store")]
fn binding_definitions() -> Result<Vec<BindingDefinition>, String> {
    Ok(read_config()?
        .custom_bindings
        .into_iter()
        .map(|binding| {
            let resolved = crate::sandbox::resolve_security_bookmark(&binding.security_bookmark);
            let detected = resolved.as_ref().is_ok_and(|path| path.is_dir());
            let source = resolved.unwrap_or_else(|_| PathBuf::from(&binding.source_path));
            BindingDefinition {
                id: binding.id,
                tool: binding.tool,
                kind: binding.kind,
                detected,
                source,
                custom: true,
            }
        })
        .collect())
}

/// 返回当前设备中允许被“刷新本机资产”读取的显式工具入口。
/// 这里只暴露路径元数据；扫描器不跟随符号链接，也不会修改这些目录。
#[cfg(all(feature = "public-release", not(feature = "app-store")))]
pub(crate) fn local_asset_roots() -> Result<Vec<(String, String, PathBuf)>, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法定位当前用户主目录".to_string())?;
    let mut roots = binding_definitions()?
        .into_iter()
        .filter(|definition| definition.source.exists())
        .map(|definition| (definition.tool, definition.kind, definition.source))
        .collect::<Vec<_>>();
    // Vendor/package locations are discovery-only. They are not path-switch targets, so a
    // manual refresh can find them without offering to rewrite vendor-managed directories.
    for (tool, root) in [
        ("Codex 插件", home.join(".codex/plugins/cache")),
        ("Trae CN 内置 Skill", home.join(".trae-cn/builtin_skills")),
        ("Trae CN 设计库", home.join(".trae-cn/design_libraries")),
    ] {
        if root.exists() {
            roots.push((tool.to_string(), "Skill".to_string(), root));
        }
    }
    Ok(roots)
}

#[cfg(feature = "app-store")]
pub(crate) fn local_asset_roots() -> Result<Vec<(String, String, PathBuf)>, String> {
    Ok(binding_definitions()?
        .into_iter()
        .filter(|definition| definition.detected && definition.source.is_dir())
        .map(|definition| (definition.tool, definition.kind, definition.source))
        .collect())
}

#[cfg(not(feature = "app-store"))]
fn target_for(definition: &BindingDefinition) -> Result<PathBuf, String> {
    Ok(unified_root()?.join(if definition.kind == "Agent" {
        "agents"
    } else {
        "skills"
    }))
}

fn entry_count(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|entries| entries.flatten().count())
        .unwrap_or(0)
}

#[cfg(not(feature = "app-store"))]
fn preview_binding(definition: &BindingDefinition) -> Result<ToolBindingPreview, String> {
    let target = target_for(definition)?;
    let metadata = fs::symlink_metadata(&definition.source).ok();
    let (source_state, status, can_apply, requires_migration, message) = match metadata {
        Some(metadata) if metadata.file_type().is_symlink() => {
            let linked = fs::read_link(&definition.source).ok();
            if linked.as_deref() == Some(target.as_path()) {
                (
                    "symlink".to_string(),
                    "unified".to_string(),
                    false,
                    false,
                    "已由灵栈统一管理".to_string(),
                )
            } else {
                (
                    "symlink".to_string(),
                    "different_link".to_string(),
                    definition.detected,
                    true,
                    "当前指向其他位置，应用前会备份原链接".to_string(),
                )
            }
        }
        Some(metadata) if metadata.is_dir() => {
            let count = entry_count(&definition.source);
            if count == 0 {
                (
                    "empty_directory".to_string(),
                    "ready".to_string(),
                    definition.detected,
                    false,
                    "空目录可安全切换到统一入口".to_string(),
                )
            } else {
                (
                    "directory".to_string(),
                    "migration_required".to_string(),
                    definition.detected,
                    true,
                    format!("现有 {count} 个顶层条目，需先无冲突迁移并备份"),
                )
            }
        }
        Some(_) => (
            "file".to_string(),
            "blocked".to_string(),
            false,
            true,
            "入口被普通文件占用，无法自动切换".to_string(),
        ),
        None if definition.detected => (
            "missing".to_string(),
            "ready".to_string(),
            true,
            false,
            "工具已发现，可创建统一入口".to_string(),
        ),
        None => (
            "missing".to_string(),
            "not_detected".to_string(),
            false,
            false,
            "未探测到该工具，不会提前创建伪入口".to_string(),
        ),
    };

    Ok(ToolBindingPreview {
        id: definition.id.to_string(),
        tool: definition.tool.to_string(),
        kind: definition.kind.to_string(),
        source_path: definition.source.display().to_string(),
        target_path: target.display().to_string(),
        source_state,
        entry_count: entry_count(&definition.source),
        detected: definition.detected,
        status,
        can_apply,
        requires_migration,
        message,
        custom: definition.custom,
    })
}

#[cfg(feature = "app-store")]
fn preview_binding(definition: &BindingDefinition) -> Result<ToolBindingPreview, String> {
    let exists = definition.detected;
    Ok(ToolBindingPreview {
        id: definition.id.clone(),
        tool: definition.tool.clone(),
        kind: definition.kind.clone(),
        source_path: definition.source.display().to_string(),
        target_path: String::new(),
        source_state: if exists { "selected" } else { "missing" }.to_string(),
        entry_count: entry_count(&definition.source),
        detected: exists,
        status: if exists { "authorized" } else { "unavailable" }.to_string(),
        can_apply: false,
        requires_migration: false,
        message: if exists {
            "已获沙盒授权；只在手动刷新时读取，不会改写工具路径".to_string()
        } else {
            "所选目录当前不可用，请删除后重新选择".to_string()
        },
        custom: definition.custom,
    })
}

fn read_receipts() -> Result<Vec<BindingReceipt>, String> {
    let path = control_dir()?.join(RECEIPTS_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(path).map_err(|error| format!("无法读取路径回执：{error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("路径回执结构无效：{error}"))
}

#[tauri::command]
pub fn load_control_center() -> Result<ControlCenterState, String> {
    let config = read_config()?;
    let bindings = binding_definitions()?
        .iter()
        .map(preview_binding)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ControlCenterState {
        profiles: config.profiles,
        bindings,
        receipts: read_receipts()?.into_iter().rev().take(20).collect(),
        unified_root: unified_root()?.display().to_string(),
        store_sandbox: cfg!(feature = "app-store"),
    })
}

#[tauri::command]
pub fn save_model_profile(
    mut profile: ModelProfile,
    secret: String,
) -> Result<ControlCenterState, String> {
    profile.endpoint = normalize_endpoint_input(&profile.endpoint);
    normalize_profile_models(&mut profile);
    validate_profile(&profile)?;
    if profile.models.is_empty() {
        return Err("请先拉取并选择至少一个模型，或手动填写模型名称".to_string());
    }
    if secret.len() > 8192 {
        return Err("模型密钥长度超过安全上限".to_string());
    }
    #[cfg(feature = "app-store")]
    if !secret.is_empty() {
        return Err("Mac App Store 版不接收模型密钥".to_string());
    }
    #[cfg(feature = "app-store")]
    {
        profile.api_key_env.clear();
        profile.credential_stored = false;
        delete_keychain_secret(&profile.id);
    }
    #[cfg(not(feature = "app-store"))]
    if !secret.is_empty() {
        store_keychain_secret(&profile.id, &secret)?;
        profile.credential_stored = true;
    } else {
        profile.credential_stored = keychain_secret(&profile.id).is_some();
    }
    let mut config = read_config()?;
    if let Some(existing) = config
        .profiles
        .iter_mut()
        .find(|item| item.id == profile.id)
    {
        *existing = profile;
    } else {
        config.profiles.push(profile);
    }
    write_json_atomic(&control_dir()?.join(CONFIG_FILE), &config)?;
    load_control_center()
}

#[tauri::command]
pub fn delete_model_profile(id: String) -> Result<ControlCenterState, String> {
    if id == "ollama-local" {
        return Err("内置 Ollama 配置可以编辑，但不能删除".to_string());
    }
    let mut config = read_config()?;
    config.profiles.retain(|profile| profile.id != id);
    write_json_atomic(&control_dir()?.join(CONFIG_FILE), &config)?;
    delete_keychain_secret(&id);
    load_control_center()
}

fn credential_confirmation_matches(id: &str, confirmation: &str) -> bool {
    confirmation == format!("CLEAR_CREDENTIAL:{id}")
}

#[tauri::command]
pub fn clear_model_credential(
    id: String,
    confirmation: String,
) -> Result<ControlCenterState, String> {
    if !credential_confirmation_matches(&id, &confirmation) {
        return Err("清除密钥缺少精确确认".to_string());
    }
    if !read_config()?
        .profiles
        .iter()
        .any(|profile| profile.id == id)
    {
        return Err("未找到模型配置".to_string());
    }
    delete_keychain_secret(&id);
    load_control_center()
}

fn validate_custom_binding(binding: &CustomToolBinding) -> Result<(), String> {
    if !binding.id.starts_with("custom-")
        || !binding
            .id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("自定义绑定 ID 格式无效".to_string());
    }
    if binding.tool.trim().is_empty() || binding.tool.chars().count() > 48 {
        return Err("工具名称不能为空且不能超过 48 个字符".to_string());
    }
    if binding.kind != "Agent" && binding.kind != "Skill" {
        return Err("资产类型只能是 Agent 或 Skill".to_string());
    }
    let path = Path::new(&binding.source_path);
    #[cfg(not(feature = "app-store"))]
    {
        let home = dirs::home_dir().ok_or_else(|| "无法定位当前用户主目录".to_string())?;
        if !path.is_absolute()
            || !path.starts_with(&home)
            || path == home
            || path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err("自定义入口必须是主目录下的明确绝对路径，且不能包含 ..".to_string());
        }
    }
    #[cfg(feature = "app-store")]
    if !path.is_absolute()
        || path.parent().is_none()
        || !path.is_dir()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("请选择一个当前可访问的明确文件夹".to_string());
    }
    if path.starts_with(unified_root()?) {
        return Err("工具入口不能位于灵栈统一目录内部，以免形成循环链接".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn save_custom_binding(binding: CustomToolBinding) -> Result<ControlCenterState, String> {
    #[cfg(feature = "app-store")]
    let mut binding = binding;
    validate_custom_binding(&binding)?;
    #[cfg(feature = "app-store")]
    {
        binding.security_bookmark =
            crate::sandbox::create_security_bookmark(Path::new(&binding.source_path))?;
    }
    let mut config = read_config()?;
    if config
        .custom_bindings
        .iter()
        .any(|item| item.source_path == binding.source_path && item.id != binding.id)
    {
        return Err("该工具入口已经登记".to_string());
    }
    if let Some(existing) = config
        .custom_bindings
        .iter_mut()
        .find(|item| item.id == binding.id)
    {
        *existing = binding;
    } else {
        config.custom_bindings.push(binding);
    }
    write_json_atomic(&control_dir()?.join(CONFIG_FILE), &config)?;
    load_control_center()
}

#[tauri::command]
pub fn delete_custom_binding(id: String) -> Result<ControlCenterState, String> {
    if read_receipts()?
        .iter()
        .any(|receipt| receipt.binding_id == id && receipt.status == "applied")
    {
        return Err("该入口仍有已应用回执，请先恢复原路径再删除登记".to_string());
    }
    let mut config = read_config()?;
    let before = config.custom_bindings.len();
    config.custom_bindings.retain(|binding| binding.id != id);
    if config.custom_bindings.len() == before {
        return Err("未找到自定义工具入口".to_string());
    }
    write_json_atomic(&control_dir()?.join(CONFIG_FILE), &config)?;
    load_control_center()
}

fn endpoint(profile: &ModelProfile, suffix: &str) -> String {
    let mut base = profile.endpoint.trim().trim_end_matches('/');
    #[cfg(feature = "app-store")]
    let operations = ["/api/tags", "/api/chat"];
    #[cfg(not(feature = "app-store"))]
    let operations = ["/chat/completions", "/models", "/api/tags", "/api/chat"];
    for operation in operations {
        if base.ends_with(operation) {
            base = base.trim_end_matches(operation).trim_end_matches('/');
            break;
        }
    }
    format!("{base}{suffix}")
}

async fn response_json(response: reqwest::Response) -> Result<Value, String> {
    if let Some(length) = response.content_length() {
        if length > 1_048_576 {
            return Err("模型服务响应超过 1 MiB 安全上限".to_string());
        }
    }
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取模型服务响应失败：{error}"))?;
    if bytes.len() > 1_048_576 {
        return Err("模型服务响应超过 1 MiB 安全上限".to_string());
    }
    if !status.is_success() {
        let detail = match status.as_u16() {
            401 => "：未提供密钥、密钥无效，或该密钥无权列出模型",
            403 => "：当前密钥没有列出模型的权限",
            404 => "：未找到模型列表接口，请检查基础 URL 与 /v1 前缀",
            429 => "：服务限流，请稍后重试",
            _ => "",
        };
        return Err(format!("模型服务返回 HTTP {}{detail}", status.as_u16()));
    }
    serde_json::from_slice(&bytes).map_err(|_| "模型服务没有返回有效 JSON".to_string())
}

#[cfg(not(feature = "app-store"))]
fn resolve_secret(profile: &ModelProfile, supplied: &str) -> Result<Option<String>, String> {
    if !supplied.is_empty() {
        return Ok(Some(supplied.to_string()));
    }
    if let Some(secret) = keychain_secret(&profile.id) {
        return Ok(Some(secret));
    }
    if profile.api_key_env.is_empty() {
        return Ok(None);
    }
    std::env::var(&profile.api_key_env).map(Some).map_err(|_| {
        format!(
            "没有找到钥匙串密钥，当前应用进程也未读取到环境变量 {}",
            profile.api_key_env
        )
    })
}

fn parse_model_options(value: &Value, _provider: &str) -> Result<Vec<ModelOption>, String> {
    #[cfg(feature = "app-store")]
    let rows = value
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| "模型服务响应中没有可识别的模型列表".to_string())?;
    #[cfg(not(feature = "app-store"))]
    let rows = if _provider == "ollama" {
        value.get("models")
    } else {
        value.get("data")
    }
    .and_then(Value::as_array)
    .ok_or_else(|| "模型服务响应中没有可识别的模型列表".to_string())?;
    let mut options = rows
        .iter()
        .filter_map(|row| {
            let id = row
                .get("id")
                .or_else(|| row.get("name"))
                .or_else(|| row.get("model"))
                .and_then(Value::as_str)?
                .trim()
                .to_string();
            if id.is_empty() {
                return None;
            }
            let owner = row.get("owned_by").and_then(Value::as_str).unwrap_or("");
            Some(ModelOption {
                label: if owner.is_empty() {
                    id.clone()
                } else {
                    format!("{id} · {owner}")
                },
                id,
            })
        })
        .take(500)
        .collect::<Vec<_>>();
    options.sort_by(|left, right| left.id.cmp(&right.id));
    options.dedup_by(|left, right| left.id == right.id);
    Ok(options)
}

#[tauri::command]
pub async fn list_model_options(
    mut profile: ModelProfile,
    secret: String,
) -> Result<Vec<ModelOption>, String> {
    profile.endpoint = normalize_endpoint_input(&profile.endpoint);
    validate_profile(&profile)?;
    if secret.len() > 8192 {
        return Err("模型密钥长度超过安全上限".to_string());
    }
    #[cfg(feature = "app-store")]
    if !secret.is_empty() {
        return Err("Mac App Store 版不接收模型密钥".to_string());
    }
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| format!("无法建立模型客户端：{error}"))?;
    #[cfg(feature = "app-store")]
    let request = client.get(endpoint(&profile, "/api/tags"));
    #[cfg(not(feature = "app-store"))]
    let mut request = if profile.provider == "ollama" {
        client.get(endpoint(&profile, "/api/tags"))
    } else {
        client.get(endpoint(&profile, "/models"))
    };
    #[cfg(not(feature = "app-store"))]
    if let Some(secret) = resolve_secret(&profile, &secret)? {
        request = request.bearer_auth(secret);
    }
    let value = response_json(
        request
            .send()
            .await
            .map_err(|error| format!("拉取模型列表失败：{error}"))?,
    )
    .await?;
    parse_model_options(&value, &profile.provider)
}

#[tauri::command]
pub async fn test_model_profile(id: String, inference: bool) -> Result<ModelTestResult, String> {
    let profile = read_config()?
        .profiles
        .into_iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| "未找到模型配置".to_string())?;
    validate_profile(&profile)?;
    if !profile.enabled {
        return Err("该模型配置已停用".to_string());
    }
    #[cfg(feature = "app-store")]
    let key = None;
    #[cfg(not(feature = "app-store"))]
    let key = resolve_secret(&profile, "")?;
    test_profile_connection(profile, key, inference).await
}

#[cfg(feature = "app-store")]
async fn test_profile_connection(
    profile: ModelProfile,
    _key: Option<String>,
    inference: bool,
) -> Result<ModelTestResult, String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(if inference { 20 } else { 8 }))
        .build()
        .map_err(|error| format!("无法建立模型客户端：{error}"))?;
    let started = Instant::now();
    let request = if inference {
        client.post(endpoint(&profile, "/api/chat")).json(&json!({
            "model": profile.model,
            "messages": [{"role": "user", "content": "Reply with OK only."}],
            "stream": false,
            "options": {"num_predict": 4}
        }))
    } else {
        client.get(endpoint(&profile, "/api/tags"))
    };
    let value = response_json(
        request
            .send()
            .await
            .map_err(|error| format!("模型连接失败：{error}"))?,
    )
    .await?;
    let summary = if inference {
        let content = value
            .pointer("/message/content")
            .and_then(Value::as_str)
            .unwrap_or("推理成功，服务未返回可展示文本");
        format!(
            "最小推理成功：{}",
            content.chars().take(80).collect::<String>()
        )
    } else {
        let count = value
            .get("models")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        format!("连接成功，服务报告 {count} 个可见模型")
    };
    Ok(ModelTestResult {
        profile_id: profile.id,
        mode: if inference {
            "inference"
        } else {
            "connectivity"
        }
        .to_string(),
        status: "success".to_string(),
        latency_ms: started.elapsed().as_millis(),
        summary,
        checked_at: Local::now().to_rfc3339(),
    })
}

#[cfg(not(feature = "app-store"))]
async fn test_profile_connection(
    profile: ModelProfile,
    key: Option<String>,
    inference: bool,
) -> Result<ModelTestResult, String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(if inference { 20 } else { 8 }))
        .build()
        .map_err(|error| format!("无法建立模型客户端：{error}"))?;
    let started = Instant::now();
    let mut request = if inference {
        if profile.provider == "ollama" {
            client.post(endpoint(&profile, "/api/chat")).json(&json!({
                "model": profile.model,
                "messages": [{"role": "user", "content": "Reply with OK only."}],
                "stream": false,
                "options": {"num_predict": 4}
            }))
        } else {
            client
                .post(endpoint(&profile, "/chat/completions"))
                .json(&json!({
                    "model": profile.model,
                    "messages": [{"role": "user", "content": "Reply with OK only."}],
                    "max_tokens": 4,
                    "temperature": 0
                }))
        }
    } else if profile.provider == "ollama" {
        client.get(endpoint(&profile, "/api/tags"))
    } else {
        client.get(endpoint(&profile, "/models"))
    };
    if let Some(secret) = key {
        request = request.bearer_auth(secret);
    }
    let value = response_json(
        request
            .send()
            .await
            .map_err(|error| format!("模型连接失败：{error}"))?,
    )
    .await?;
    let summary = if inference {
        let content = if profile.provider == "ollama" {
            value.pointer("/message/content")
        } else {
            value.pointer("/choices/0/message/content")
        }
        .and_then(Value::as_str)
        .unwrap_or("推理成功，服务未返回可展示文本");
        format!(
            "最小推理成功：{}",
            content.chars().take(80).collect::<String>()
        )
    } else {
        let count = if profile.provider == "ollama" {
            value.get("models")
        } else {
            value.get("data")
        }
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
        format!("连接成功，服务报告 {count} 个可见模型")
    };
    Ok(ModelTestResult {
        profile_id: profile.id,
        mode: if inference {
            "inference"
        } else {
            "connectivity"
        }
        .to_string(),
        status: "success".to_string(),
        latency_ms: started.elapsed().as_millis(),
        summary,
        checked_at: Local::now().to_rfc3339(),
    })
}

#[cfg(not(feature = "app-store"))]
fn file_hash(path: &Path) -> Result<Vec<u8>, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("无法比较文件 {}：{error}", path.display()))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(format!(
            "冲突文件超过 4 MiB，拒绝自动合并：{}",
            path.display()
        ));
    }
    Ok(Sha256::digest(bytes).to_vec())
}

#[cfg(not(feature = "app-store"))]
fn preflight_merge(source: &Path, target: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Ok(());
    }
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let relative = entry.path().strip_prefix(source).unwrap_or(entry.path());
        if relative.as_os_str().is_empty() {
            continue;
        }
        let destination = target.join(relative);
        if !destination.exists() && fs::symlink_metadata(&destination).is_err() {
            continue;
        }
        let source_metadata =
            fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
        let target_metadata =
            fs::symlink_metadata(&destination).map_err(|error| error.to_string())?;
        if source_metadata.is_dir() && target_metadata.is_dir() {
            continue;
        }
        if source_metadata.is_file()
            && target_metadata.is_file()
            && file_hash(entry.path())? == file_hash(&destination)?
        {
            continue;
        }
        if source_metadata.file_type().is_symlink()
            && target_metadata.file_type().is_symlink()
            && fs::read_link(entry.path()).ok() == fs::read_link(&destination).ok()
        {
            continue;
        }
        return Err(format!(
            "统一目录存在不同内容，已在写入前停止：{}",
            destination.display()
        ));
    }
    Ok(())
}

#[cfg(not(feature = "app-store"))]
fn copy_merge(source: &Path, target: &Path) -> Result<(), String> {
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let relative = entry.path().strip_prefix(source).unwrap_or(entry.path());
        if relative.as_os_str().is_empty() {
            continue;
        }
        let destination = target.join(relative);
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            fs::create_dir_all(&destination)
                .map_err(|error| format!("无法创建统一目录：{error}"))?;
        } else if metadata.is_file() && !destination.exists() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(entry.path(), &destination)
                .map_err(|error| format!("无法复制现有资产：{error}"))?;
        } else if metadata.file_type().is_symlink() && fs::symlink_metadata(&destination).is_err() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(
                fs::read_link(entry.path()).map_err(|error| error.to_string())?,
                &destination,
            )
            .map_err(|error| format!("无法复制符号链接：{error}"))?;
        }
    }
    Ok(())
}

#[cfg(not(feature = "app-store"))]
fn definition_by_id(id: &str) -> Result<BindingDefinition, String> {
    binding_definitions()?
        .into_iter()
        .find(|definition| definition.id == id)
        .ok_or_else(|| "未知的工具路径绑定；只允许修改已登记的内置或自定义入口".to_string())
}

#[tauri::command]
#[cfg(not(feature = "app-store"))]
pub fn apply_tool_binding(
    binding_id: String,
    migrate_existing: bool,
    confirmation: String,
) -> Result<BindingReceipt, String> {
    if confirmation != format!("APPLY:{binding_id}") {
        return Err("路径变更缺少精确确认".to_string());
    }
    let definition = definition_by_id(&binding_id)?;
    let preview = preview_binding(&definition)?;
    if !preview.can_apply {
        return Err(preview.message);
    }
    if preview.requires_migration && !migrate_existing {
        return Err("现有内容必须选择“迁移并备份”后才能切换".to_string());
    }
    let target = target_for(&definition)?;
    fs::create_dir_all(&target).map_err(|error| format!("无法创建统一目录：{error}"))?;
    if definition.source.is_dir() {
        preflight_merge(&definition.source, &target)?;
        copy_merge(&definition.source, &target)?;
    }
    if let Some(parent) = definition.source.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建工具配置目录：{error}"))?;
    }
    let receipt_id = format!("{}-{}", Local::now().format("%Y%m%d%H%M%S"), binding_id);
    let backup = control_dir()?.join("backups").join(&receipt_id);
    let had_source = fs::symlink_metadata(&definition.source).is_ok();
    if had_source {
        fs::create_dir_all(backup.parent().unwrap()).map_err(|error| error.to_string())?;
        fs::rename(&definition.source, &backup)
            .map_err(|error| format!("无法备份原入口：{error}"))?;
    }
    #[cfg(unix)]
    if let Err(error) = std::os::unix::fs::symlink(&target, &definition.source) {
        if had_source {
            let _ = fs::rename(&backup, &definition.source);
        }
        return Err(format!("创建统一入口失败，原入口已恢复：{error}"));
    }
    let receipt = BindingReceipt {
        id: receipt_id,
        binding_id,
        source_path: definition.source.display().to_string(),
        target_path: target.display().to_string(),
        prior_state: preview.source_state,
        backup_path: if had_source {
            backup.display().to_string()
        } else {
            String::new()
        },
        applied_at: Local::now().to_rfc3339(),
        status: "applied".to_string(),
    };
    let mut receipts = read_receipts()?;
    receipts.push(receipt.clone());
    write_json_atomic(&control_dir()?.join(RECEIPTS_FILE), &receipts)?;
    Ok(receipt)
}

#[tauri::command]
#[cfg(feature = "app-store")]
pub fn apply_tool_binding(
    _binding_id: String,
    _migrate_existing: bool,
    _confirmation: String,
) -> Result<BindingReceipt, String> {
    Err("App Store 沙盒版不会更改任何工具的调用路径；请使用目录选择授权读取".to_string())
}

#[tauri::command]
#[cfg(not(feature = "app-store"))]
pub fn restore_tool_binding(
    receipt_id: String,
    confirmation: String,
) -> Result<BindingReceipt, String> {
    if confirmation != format!("RESTORE:{receipt_id}") {
        return Err("恢复操作缺少精确确认".to_string());
    }
    let mut receipts = read_receipts()?;
    let index = receipts
        .iter()
        .position(|receipt| receipt.id == receipt_id)
        .ok_or_else(|| "未找到路径变更回执".to_string())?;
    let mut receipt = receipts[index].clone();
    if receipt.status != "applied" {
        return Err("该回执已经恢复或不可恢复".to_string());
    }
    let definition = definition_by_id(&receipt.binding_id)?;
    let current = fs::symlink_metadata(&definition.source)
        .map_err(|_| "当前工具入口不存在，拒绝猜测恢复".to_string())?;
    if !current.file_type().is_symlink()
        || fs::read_link(&definition.source).ok().as_deref()
            != Some(Path::new(&receipt.target_path))
    {
        return Err("当前入口不再指向该回执的统一目录，拒绝覆盖后续改动".to_string());
    }
    fs::remove_file(&definition.source).map_err(|error| format!("无法移除统一链接：{error}"))?;
    if !receipt.backup_path.is_empty() {
        if let Err(error) = fs::rename(&receipt.backup_path, &definition.source) {
            #[cfg(unix)]
            let _ = std::os::unix::fs::symlink(&receipt.target_path, &definition.source);
            return Err(format!("恢复备份失败，统一链接已回建：{error}"));
        }
    }
    receipt.status = "restored".to_string();
    receipts[index] = receipt.clone();
    write_json_atomic(&control_dir()?.join(RECEIPTS_FILE), &receipts)?;
    Ok(receipt)
}

#[tauri::command]
#[cfg(feature = "app-store")]
pub fn restore_tool_binding(
    _receipt_id: String,
    _confirmation: String,
) -> Result<BindingReceipt, String> {
    Err("App Store 沙盒版没有路径切换操作，因此无需恢复".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_validation_rejects_secret_like_invalid_env_name() {
        let mut profile = default_profiles().remove(0);
        profile.api_key_env = "KEY=value".to_string();
        assert!(validate_profile(&profile).is_err());
    }

    #[test]
    fn profile_validation_rejects_file_urls() {
        let mut profile = default_profiles().remove(0);
        profile.endpoint = "file:///tmp/model".to_string();
        assert!(validate_profile(&profile).is_err());
    }

    #[test]
    #[cfg(not(feature = "app-store"))]
    fn merge_preflight_accepts_identical_files_and_rejects_conflicts() {
        let root =
            std::env::temp_dir().join(format!("lingzhan-control-test-{}", std::process::id()));
        let source = root.join("source");
        let target = root.join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("asset.md"), "same").unwrap();
        fs::write(target.join("asset.md"), "same").unwrap();
        assert!(preflight_merge(&source, &target).is_ok());
        fs::write(target.join("asset.md"), "different").unwrap();
        assert!(preflight_merge(&source, &target).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(not(feature = "app-store"))]
    fn model_list_parser_supports_openai_and_ollama_shapes() {
        let openai = parse_model_options(
            &json!({"data": [{"id": "gpt-b", "owned_by": "local"}, {"id": "gpt-a"}]}),
            "openai_compatible",
        )
        .unwrap();
        assert_eq!(openai[0].id, "gpt-a");
        assert_eq!(openai[1].label, "gpt-b · local");
        let ollama =
            parse_model_options(&json!({"models": [{"name": "qwen3:8b"}]}), "ollama").unwrap();
        assert_eq!(ollama[0].id, "qwen3:8b");
    }

    #[test]
    fn endpoint_normalization_cleans_port_commas_only() {
        assert_eq!(
            normalize_endpoint_input("http://127.0.0.1:11,435/v1"),
            "http://127.0.0.1:11435/v1"
        );
        assert_eq!(
            normalize_endpoint_input("https://example.com/v1,preview"),
            "https://example.com/v1,preview"
        );
    }

    #[test]
    #[cfg(not(feature = "app-store"))]
    fn endpoint_builder_accepts_base_or_full_models_url() {
        let mut profile = default_profiles().remove(0);
        profile.provider = "openai_compatible".to_string();
        profile.endpoint = "http://127.0.0.1:11435/v1/models".to_string();
        assert_eq!(
            endpoint(&profile, "/models"),
            "http://127.0.0.1:11435/v1/models"
        );
        assert_eq!(
            endpoint(&profile, "/chat/completions"),
            "http://127.0.0.1:11435/v1/chat/completions"
        );
    }

    #[test]
    fn credential_clear_requires_exact_profile_confirmation() {
        assert!(credential_confirmation_matches(
            "model-local",
            "CLEAR_CREDENTIAL:model-local"
        ));
        assert!(!credential_confirmation_matches(
            "model-local",
            "CLEAR_CREDENTIAL:another"
        ));
    }

    fn serve_json_once(
        expected_request_line: &'static str,
        expected_authorization: Option<&'static str>,
        body: &'static str,
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = vec![0_u8; 16 * 1024];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with(expected_request_line), "{request}");
            if let Some(expected) = expected_authorization {
                assert!(request
                    .lines()
                    .any(|line| line.eq_ignore_ascii_case(expected)));
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        (endpoint, handle)
    }

    #[test]
    #[cfg(not(feature = "app-store"))]
    fn model_list_command_fetches_multiple_models_over_real_http() {
        let (endpoint, server) = serve_json_once(
            "GET /v1/models HTTP/1.1",
            Some("authorization: Bearer test-secret"),
            r#"{"data":[{"id":"model-b"},{"id":"model-a","owned_by":"local"}]}"#,
        );
        let mut profile = default_profiles().remove(0);
        profile.id = "mock-openai".to_string();
        profile.provider = "openai_compatible".to_string();
        profile.endpoint = endpoint;
        profile.model = String::new();
        profile.models.clear();
        let options =
            tauri::async_runtime::block_on(list_model_options(profile, "test-secret".to_string()))
                .unwrap();
        assert_eq!(
            options
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["model-a", "model-b"]
        );
        server.join().unwrap();
    }

    #[test]
    #[cfg(not(feature = "app-store"))]
    fn model_inference_probe_executes_over_real_http() {
        let (endpoint, server) = serve_json_once(
            "POST /v1/chat/completions HTTP/1.1",
            None,
            r#"{"choices":[{"message":{"content":"OK"}}]}"#,
        );
        let mut profile = default_profiles().remove(0);
        profile.id = "mock-openai".to_string();
        profile.provider = "openai_compatible".to_string();
        profile.endpoint = endpoint;
        profile.model = "model-a".to_string();
        profile.models = vec!["model-a".to_string()];
        let result =
            tauri::async_runtime::block_on(test_profile_connection(profile, None, true)).unwrap();
        assert_eq!(result.status, "success");
        assert!(result.summary.contains("OK"));
        server.join().unwrap();
    }

    #[test]
    #[cfg(feature = "app-store")]
    fn app_store_profiles_are_local_only_and_credential_free() {
        for endpoint in [
            "http://localhost:11434",
            "http://127.0.0.1:11434",
            "http://[::1]:11434",
        ] {
            let mut profile = default_profiles().remove(0);
            profile.endpoint = endpoint.to_string();
            assert!(validate_profile(&profile).is_ok(), "{endpoint}");
        }

        let mut remote = default_profiles().remove(0);
        remote.endpoint = "https://models.example.com".to_string();
        assert!(validate_profile(&remote).is_err());

        let mut credentialed = default_profiles().remove(0);
        credentialed.api_key_env = "MODEL_KEY".to_string();
        assert!(validate_profile(&credentialed).is_err());

        let mut unsupported = default_profiles().remove(0);
        unsupported.provider = "remote-compatible".to_string();
        assert!(validate_profile(&unsupported).is_err());
    }

    #[test]
    #[cfg(feature = "app-store")]
    fn app_store_model_list_uses_only_the_local_ollama_shape() {
        let (endpoint, server) = serve_json_once(
            "GET /v1/api/tags HTTP/1.1",
            None,
            r#"{"models":[{"name":"model-b"},{"name":"model-a"}]}"#,
        );
        let mut profile = default_profiles().remove(0);
        profile.endpoint = endpoint;
        profile.model.clear();
        profile.models.clear();
        let options =
            tauri::async_runtime::block_on(list_model_options(profile, String::new())).unwrap();
        assert_eq!(
            options
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["model-a", "model-b"]
        );
        server.join().unwrap();
    }
}
