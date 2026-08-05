use base64::{engine::general_purpose::STANDARD, Engine as _};
use objc2_foundation::{
    NSData, NSString, NSURLBookmarkCreationOptions, NSURLBookmarkResolutionOptions, NSURL,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static ACTIVE_BOOKMARKS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub fn create_security_bookmark(path: &Path) -> Result<String, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "所选目录不是有效的 UTF-8 路径".to_string())?;
    let path = NSString::from_str(path);
    let url = NSURL::fileURLWithPath_isDirectory(&path, true);
    let data = url
        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
            NSURLBookmarkCreationOptions::WithSecurityScope,
            None,
            None,
        )
        .map_err(|error| format!("无法保存目录授权：{error:?}"))?;
    Ok(STANDARD.encode(data.to_vec()))
}

pub fn resolve_security_bookmark(encoded: &str) -> Result<PathBuf, String> {
    if encoded.is_empty() {
        return Err("目录缺少沙盒授权，请重新选择".to_string());
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "目录授权数据已损坏，请重新选择".to_string())?;
    let data = NSData::with_bytes(&bytes);
    let url = unsafe {
        NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
            &data,
            NSURLBookmarkResolutionOptions::WithSecurityScope,
            None,
            std::ptr::null_mut(),
        )
    }
    .map_err(|error| format!("无法恢复目录授权：{error:?}"))?;
    let path = url
        .path()
        .map(|value| PathBuf::from(value.to_string()))
        .ok_or_else(|| "目录授权没有返回有效路径".to_string())?;

    let active = ACTIVE_BOOKMARKS.get_or_init(|| Mutex::new(HashSet::new()));
    let mut active = active
        .lock()
        .map_err(|_| "目录授权状态暂时不可用".to_string())?;
    if !active.contains(encoded) {
        if !unsafe { url.startAccessingSecurityScopedResource() } {
            return Err("macOS 拒绝访问所选目录，请重新授权".to_string());
        }
        active.insert(encoded.to_string());
        // Security-scoped access must remain active while the process uses this directory.
        // The number of retained URLs is bounded by the user's explicit bindings.
        std::mem::forget(url);
    }
    Ok(path)
}
