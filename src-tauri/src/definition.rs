//! Read-only definition access. Registry identities are the only accepted capability.
use crate::{registry::read_registry, types::Registry};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

#[cfg(any(test, not(unix)))]
use std::fs;

const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_SEARCH_FILES: usize = 2_000;
const MAX_SEARCH_BYTES: usize = 32 * 1024 * 1024;
const MAX_MATCHES: usize = 100;
static ACTIVE_READERS: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_SEARCHES: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Serialize)]
pub struct DefinitionDocument {
    pub logical_id: String,
    pub sha256: String,
    pub path: String,
    pub content: String,
    pub byte_count: usize,
    pub line_count: usize,
    pub estimated_tokens: usize,
}
#[derive(Debug, Serialize)]
pub struct DefinitionMatch {
    pub logical_id: String,
    pub sha256: String,
    pub path: String,
    pub snippet: String,
}
#[derive(Debug, Default, Serialize)]
pub struct DefinitionSearchResult {
    pub matches: Vec<DefinitionMatch>,
    pub scanned_files: usize,
    pub skipped_files: usize,
    pub truncated: bool,
}
struct Permit(&'static AtomicUsize);
impl Permit {
    fn acquire(counter: &'static AtomicUsize, limit: usize) -> Result<Self, String> {
        counter
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < limit).then_some(n + 1)
            })
            .map(|_| Self(counter))
            .map_err(|_| "原文读取任务繁忙，请稍后重试".to_string())
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

fn validate_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err("定义路径必须是无跳转的绝对路径".into());
    }
    Ok(())
}

// Walk with directory descriptors so replacing a parent with a symlink cannot redirect reads.
#[cfg(unix)]
pub(crate) fn open_without_links(path: &Path) -> Result<File, String> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
    };
    unsafe extern "C" {
        fn openat(fd: i32, path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    }
    #[cfg(target_os = "macos")]
    const NOFOLLOW: i32 = 0x100;
    #[cfg(target_os = "macos")]
    const DIRECTORY: i32 = 0x100000;
    #[cfg(not(target_os = "macos"))]
    const NOFOLLOW: i32 = 0x20000;
    #[cfg(not(target_os = "macos"))]
    const DIRECTORY: i32 = 0x10000;
    #[cfg(target_os = "macos")]
    const NONBLOCK: i32 = 0x4;
    #[cfg(not(target_os = "macos"))]
    const NONBLOCK: i32 = 0x800;
    #[cfg(target_os = "macos")]
    const CLOEXEC: i32 = 0x1000000;
    #[cfg(not(target_os = "macos"))]
    const CLOEXEC: i32 = 0x80000;
    validate_path(path)?;
    let mut handle = File::open("/").map_err(|_| "无法打开文件系统根目录".to_string())?;
    let parts: Vec<_> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p),
            _ => None,
        })
        .collect();
    for (i, part) in parts.iter().enumerate() {
        let name = CString::new(part.as_bytes()).map_err(|_| "定义路径无效".to_string())?;
        let flags = NOFOLLOW | NONBLOCK | CLOEXEC | if i + 1 < parts.len() { DIRECTORY } else { 0 };
        // SAFETY: live directory fd, NUL-terminated name, read-only flags, ownership transferred once.
        let fd = unsafe { openat(handle.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err("定义不可读或路径含符号链接".into());
        }
        handle = unsafe { File::from_raw_fd(fd) };
    }
    Ok(handle)
}
#[cfg(not(unix))]
pub(crate) fn open_without_links(path: &Path) -> Result<File, String> {
    validate_path(path)?;
    let mut prefix = PathBuf::new();
    for part in path.components() {
        prefix.push(part);
        let metadata = fs::symlink_metadata(&prefix).map_err(|_| "定义路径不可读".to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("定义路径含符号链接".into());
        }
    }
    File::open(path).map_err(|_| "定义不可读".into())
}
fn definition_path(kind: &str, location: &str) -> Result<PathBuf, String> {
    let mut path = PathBuf::from(location);
    validate_path(&path)?;
    if kind.eq_ignore_ascii_case("skill") {
        if path.file_name().is_none_or(|name| name != "SKILL.md") {
            path.push("SKILL.md");
        }
    } else if !kind.eq_ignore_ascii_case("agent") {
        return Err("仅支持 Agent 与 Skill 定义".into());
    }
    if !matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("md" | "toml" | "yaml" | "yml" | "json")
    ) {
        return Err("不支持的定义文件类型".into());
    }
    Ok(path)
}
fn read_document(
    kind: &str,
    logical_id: &str,
    sha256: &str,
    location: &str,
    budget: &mut usize,
) -> Result<DefinitionDocument, String> {
    let path = definition_path(kind, location)?;
    let file = open_without_links(&path)?;
    let metadata = file
        .metadata()
        .map_err(|_| "无法读取定义元数据".to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES as u64 {
        return Err("定义不是普通文件或超过 2 MiB".into());
    }
    let limit = MAX_FILE_BYTES.min(*budget);
    if metadata.len() as usize > limit {
        *budget = 0;
        return Err("定义超过本次读取预算".into());
    }
    let mut bytes = Vec::new();
    let result = file.take(limit as u64 + 1).read_to_end(&mut bytes);
    *budget = budget.saturating_sub(bytes.len());
    result.map_err(|_| "定义读取失败".to_string())?;
    if bytes.len() > limit {
        return Err("定义在读取时超过大小上限".into());
    }
    let actual_hash = format!("{:x}", Sha256::digest(&bytes));
    if actual_hash != sha256 {
        return Err("定义内容已变化，请刷新资产库后重试".into());
    }
    let byte_count = bytes.len();
    let content = String::from_utf8(bytes).map_err(|_| "定义不是有效 UTF-8 文本".to_string())?;
    let ascii = content.chars().filter(char::is_ascii).count();
    let estimated_tokens = ascii.div_ceil(4) + content.chars().filter(|c| !c.is_ascii()).count();
    Ok(DefinitionDocument {
        logical_id: logical_id.into(),
        sha256: actual_hash,
        path: path.to_string_lossy().into_owned(),
        line_count: content.lines().count(),
        estimated_tokens,
        byte_count,
        content,
    })
}
fn read_from_registry(
    registry: &Registry,
    logical_id: &str,
    sha256: &str,
) -> Result<DefinitionDocument, String> {
    let asset = registry
        .groups
        .iter()
        .find(|a| a.logical_id == logical_id)
        .ok_or("资产已不在当前注册表中")?;
    let variant = asset
        .variants
        .iter()
        .find(|v| v.sha256 == sha256)
        .ok_or("版本已不在当前注册表中")?;
    let mut budget = MAX_SEARCH_BYTES;
    let mut error = "当前版本没有可读定义位置".to_string();
    for location in variant.locations.iter().take(32) {
        match read_document(&asset.kind, logical_id, sha256, &location.path, &mut budget) {
            Ok(doc) => return Ok(doc),
            Err(reason) => error = reason,
        }
    }
    Err(error)
}
fn snippet(content: &str, needle: &str) -> Option<String> {
    // Locate the folded match, then map its byte offset back to the original text.
    let folded_start = content.to_lowercase().find(needle)?;
    let mut folded_offset = 0;
    let mut start = 0;
    for (index, character) in content.char_indices() {
        let length: usize = character.to_lowercase().map(char::len_utf8).sum();
        if folded_offset + length > folded_start {
            start = index;
            break;
        }
        folded_offset += length;
    }
    let prefix: String = content[..start]
        .chars()
        .rev()
        .take(60)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let tail: String = content[start..].chars().take(180).collect();
    Some(format!(
        "{}{prefix}{tail}{}",
        if prefix.len() < start { "…" } else { "" },
        if start + tail.len() < content.len() {
            "…"
        } else {
            ""
        }
    ))
}
fn search_registry(registry: &Registry, query: &str) -> DefinitionSearchResult {
    let needle = query.to_lowercase();
    let start = Instant::now();
    let mut budget = MAX_SEARCH_BYTES;
    let mut result = DefinitionSearchResult::default();
    'assets: for asset in &registry.groups {
        for variant in &asset.variants {
            for location in &variant.locations {
                if result.scanned_files + result.skipped_files >= MAX_SEARCH_FILES
                    || budget == 0
                    || start.elapsed() >= Duration::from_secs(2)
                    || result.matches.len() >= MAX_MATCHES
                {
                    result.truncated = true;
                    return result;
                }
                match read_document(
                    &asset.kind,
                    &asset.logical_id,
                    &variant.sha256,
                    &location.path,
                    &mut budget,
                ) {
                    Ok(doc) => {
                        result.scanned_files += 1;
                        if let Some(snippet) = snippet(&doc.content, &needle) {
                            result.matches.push(DefinitionMatch {
                                logical_id: doc.logical_id,
                                sha256: doc.sha256,
                                path: doc.path,
                                snippet,
                            });
                            continue 'assets;
                        }
                        // Hash verification proves all readable locations of this variant
                        // have identical content, so another copy cannot produce a match.
                        break;
                    }
                    Err(_) => {
                        result.skipped_files += 1;
                        if budget == 0 {
                            result.truncated = true;
                            return result;
                        }
                    }
                }
            }
        }
    }
    result
}
#[tauri::command]
pub async fn read_asset_definition(
    logical_id: String,
    sha256: String,
) -> Result<DefinitionDocument, String> {
    let permit = Permit::acquire(&ACTIVE_READERS, 4)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        read_from_registry(&read_registry()?, &logical_id, &sha256)
    })
    .await
    .map_err(|_| "原文读取任务异常结束".to_string())?
}
#[tauri::command]
pub async fn search_asset_definitions(query: String) -> Result<DefinitionSearchResult, String> {
    let query = query.trim().to_string();
    if query.is_empty() || query.chars().count() > 200 {
        return Err("搜索词需包含 1 至 200 个字符".into());
    }
    let permit = Permit::acquire(&ACTIVE_SEARCHES, 1)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        Ok(search_registry(&read_registry()?, &query))
    })
    .await
    .map_err(|_| "全文搜索任务异常结束".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssetGroup, Location, Variant};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "lingstack-definition-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn registry(&self, bytes: &[u8]) -> Registry {
            let path = self.0.join("SKILL.md");
            fs::write(&path, bytes).unwrap();
            Registry {
                groups: vec![AssetGroup {
                    logical_id: "skill-test".into(),
                    kind: "Skill".into(),
                    variants: vec![Variant {
                        sha256: format!("{:x}", Sha256::digest(bytes)),
                        locations: vec![Location {
                            path: path.to_string_lossy().into_owned(),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn document_statistics_and_registry_identity() {
        let f = Fixture::new();
        let r = f.registry("abcd中文\n".as_bytes());
        let hash = &r.groups[0].variants[0].sha256;
        let d = read_from_registry(&r, "skill-test", hash).unwrap();
        assert_eq!((d.byte_count, d.line_count, d.estimated_tokens), (11, 1, 4));
        assert!(read_from_registry(&r, "other", hash).is_err());
        assert!(read_from_registry(&r, "skill-test", "other").is_err());
        fs::write(f.0.join("SKILL.md"), "drift").unwrap();
        assert!(read_from_registry(&r, "skill-test", hash)
            .unwrap_err()
            .contains("变化"));
    }
    #[test]
    fn invalid_utf8_size_and_path_boundaries() {
        let f = Fixture::new();
        let r = f.registry(&[0xff]);
        assert!(
            read_from_registry(&r, "skill-test", &r.groups[0].variants[0].sha256)
                .unwrap_err()
                .contains("UTF-8")
        );
        let valid = f.registry(b"budget");
        let mut budget = 2;
        assert!(read_document(
            "Skill",
            "skill-test",
            &valid.groups[0].variants[0].sha256,
            valid.groups[0].variants[0].locations[0].path.as_str(),
            &mut budget
        )
        .is_err());
        assert_eq!(budget, 0);
        assert!(definition_path("Agent", "relative.md").is_err());
        assert!(definition_path("Agent", "/a/../b.md").is_err());
        assert!(definition_path("Agent", "/a/config.env").is_err());
        let r = f.registry(&vec![b'a'; MAX_FILE_BYTES + 1]);
        assert!(read_from_registry(&r, "skill-test", &r.groups[0].variants[0].sha256).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_file_and_parent_symlinks() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        let r = f.registry(b"safe");
        let a = &r.groups[0];
        let hash = &a.variants[0].sha256;
        let link = f.0.join("linked.md");
        symlink(f.0.join("SKILL.md"), &link).unwrap();
        let mut budget = MAX_FILE_BYTES;
        assert!(read_document("Agent", "a", hash, link.to_str().unwrap(), &mut budget).is_err());
        let parent = f.0.join("parent");
        symlink(&f.0, &parent).unwrap();
        assert!(open_without_links(&parent.join("SKILL.md")).is_err());
    }
    #[test]
    fn search_is_case_insensitive_and_reports_skips() {
        let f = Fixture::new();
        let mut r = f.registry("prefix 中文 HELLO suffix".as_bytes());
        r.groups[0].variants[0].locations.insert(
            0,
            Location {
                path: f.0.join("missing/SKILL.md").to_string_lossy().into_owned(),
                ..Default::default()
            },
        );
        let found = search_registry(&r, "hello");
        assert_eq!(
            (
                found.scanned_files,
                found.skipped_files,
                found.matches.len()
            ),
            (1, 1, 1)
        );
        assert!(found.matches[0].snippet.contains("HELLO"));
        assert!(!found.truncated);
        assert!(snippet("İ中文HELLO", "hello").unwrap().contains("HELLO"));
        assert_eq!(search_registry(&r, "absent").matches.len(), 0);
    }
    #[test]
    fn duplicate_locations_do_not_consume_later_assets_or_repeat_nonmatches() {
        let f = Fixture::new();
        let mut r = f.registry(b"match");
        let mut later = r.groups[0].clone();
        later.logical_id = "later-asset".into();
        r.groups[0].variants[0].locations =
            vec![r.groups[0].variants[0].locations[0].clone(); MAX_MATCHES + 1];
        r.groups.push(later);
        let found = search_registry(&r, "match");
        assert_eq!(found.matches.len(), 2);
        assert_eq!(found.scanned_files, 2);
        assert_eq!(found.matches[1].logical_id, "later-asset");
        assert!(!found.truncated);
        let absent = search_registry(&r, "absent");
        assert!(absent.matches.is_empty());
        assert_eq!(absent.scanned_files, 2);
        assert!(!absent.truncated);
    }
    #[test]
    fn search_caps_results_and_permit_is_released() {
        let f = Fixture::new();
        let mut r = f.registry(b"match");
        let template = r.groups[0].clone();
        r.groups = (0..=MAX_MATCHES)
            .map(|index| AssetGroup {
                logical_id: format!("skill-{index}"),
                ..template.clone()
            })
            .collect();
        let found = search_registry(&r, "match");
        assert_eq!(found.matches.len(), MAX_MATCHES);
        assert!(found.truncated);
        static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let p = Permit::acquire(&TEST_COUNTER, 1).unwrap();
        assert!(Permit::acquire(&TEST_COUNTER, 1).is_err());
        drop(p);
        assert!(Permit::acquire(&TEST_COUNTER, 1).is_ok());
    }
}
