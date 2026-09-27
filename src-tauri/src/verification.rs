use crate::registry::read_registry;
use crate::types::{AssetAuditReport, AssetAuditResult, AssetGroup, AuditCheck};
use chrono::Local;
use sha2::{Digest, Sha256};
use std::fs;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

const MAX_DEFINITION_BYTES: u64 = 2 * 1024 * 1024;
const MAX_RETURNED_REPORTS: usize = 240;

fn definition_path(kind: &str, location: &str) -> PathBuf {
    let path = PathBuf::from(location);
    if kind.eq_ignore_ascii_case("skill") && path.is_dir() {
        path.join("SKILL.md")
    } else {
        path
    }
}

fn inspect_definition(asset: &AssetGroup, deep: bool) -> (usize, usize, usize) {
    let mut visible = 0;
    let mut structured = 0;
    let mut hash_matches = 0;
    for variant in &asset.variants {
        let mut variant_visible = false;
        let mut variant_structured = false;
        let mut variant_hash_matches = false;
        let mut variant_drifted = false;
        for location in &variant.locations {
            let path = definition_path(&asset.kind, &location.path);
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_DEFINITION_BYTES {
                continue;
            }
            variant_visible = true;
            if !deep {
                variant_structured = true;
                continue;
            }
            let Ok(bytes) = fs::read(&path) else { continue };
            let content = String::from_utf8_lossy(&bytes);
            let location_structured = if asset.kind.eq_ignore_ascii_case("skill") {
                content.contains("name:") && content.contains("description:")
            } else {
                content.lines().any(|line| !line.trim().is_empty())
            };
            let actual = format!("{:x}", Sha256::digest(&bytes));
            let matches = actual == variant.sha256;
            variant_drifted |= !matches;
            variant_hash_matches |= matches;
            // A valid structure must come from the same registered-content copy.
            variant_structured |= matches && location_structured;
        }
        visible += usize::from(variant_visible);
        structured += usize::from(variant_structured);
        hash_matches += usize::from(variant_hash_matches && !variant_drifted);
    }
    (visible, structured, hash_matches)
}

fn pending_license(asset: &AssetGroup) -> bool {
    let license = asset.license.trim();
    license.is_empty() || license.contains("待") || license.contains("未知")
}

pub(crate) fn audit_asset(asset: &AssetGroup, deep: bool) -> AssetAuditReport {
    let variant_count = asset.variants.len();
    let (visible, structured, hash_matches) = inspect_definition(asset, deep);
    let mut score: i16 = 0;
    let mut checks = Vec::new();
    let mut suggestions = Vec::new();

    if variant_count > 0 && visible == variant_count {
        score += 30;
        checks.push(AuditCheck {
            name: "来源可读".to_string(),
            status: "pass".to_string(),
            evidence: format!("{visible}/{variant_count} 个内容版本有可读定义"),
        });
    } else if visible > 0 {
        score += 15;
        checks.push(AuditCheck {
            name: "来源可读".to_string(),
            status: "warn".to_string(),
            evidence: format!("仅 {visible}/{variant_count} 个内容版本可读"),
        });
        suggestions.push("修复失效路径或移除无法追溯的旧位置，再重建统一注册表。".to_string());
    } else {
        checks.push(AuditCheck {
            name: "来源可读".to_string(),
            status: "fail".to_string(),
            evidence: "没有找到可读且大小合理的定义文件".to_string(),
        });
        suggestions.push("先恢复至少一个可读来源；在此之前不要标记为可复用。".to_string());
    }

    if variant_count > 0 && structured == variant_count {
        score += 25;
        checks.push(AuditCheck {
            name: "结构完整".to_string(),
            status: "pass".to_string(),
            evidence: if deep {
                "定义满足当前最小结构合同"
            } else {
                "轻量检查确认定义文件非空"
            }
            .to_string(),
        });
    } else {
        score += if structured > 0 { 10 } else { 0 };
        checks.push(AuditCheck {
            name: "结构完整".to_string(),
            status: if structured > 0 { "warn" } else { "fail" }.to_string(),
            evidence: format!("{structured}/{variant_count} 个版本通过结构检查"),
        });
        suggestions.push(
            if asset.kind.eq_ignore_ascii_case("skill") {
                "补齐 SKILL.md 的 name、description 与清晰触发边界。"
            } else {
                "补齐 Agent 的职责、输入输出、工具权限与失败边界。"
            }
            .to_string(),
        );
    }

    if deep {
        if variant_count > 0 && hash_matches == variant_count {
            score += 15;
            checks.push(AuditCheck {
                name: "内容哈希".to_string(),
                status: "pass".to_string(),
                evidence: "当前文件与注册表 SHA-256 一致".to_string(),
            });
        } else {
            checks.push(AuditCheck {
                name: "内容哈希".to_string(),
                status: "warn".to_string(),
                evidence: format!("{hash_matches}/{variant_count} 个版本与登记哈希一致"),
            });
            suggestions.push("内容可能已变化；刷新清单与快照后再进行运行时验真。".to_string());
        }
    } else {
        score += 8;
        checks.push(AuditCheck {
            name: "内容哈希".to_string(),
            status: "skipped".to_string(),
            evidence: "轻量自动检查不读取正文；深度验真时再比对".to_string(),
        });
    }

    if variant_count == 1 {
        score += 15;
        checks.push(AuditCheck {
            name: "版本唯一".to_string(),
            status: "pass".to_string(),
            evidence: "当前只有一个内容版本".to_string(),
        });
    } else {
        checks.push(AuditCheck {
            name: "版本唯一".to_string(),
            status: "warn".to_string(),
            evidence: format!("存在 {variant_count} 个不同内容版本"),
        });
        suggestions.push("比较差异并固定一个验证版本；不要让工具随机命中不同副本。".to_string());
    }

    if pending_license(asset) {
        checks.push(AuditCheck {
            name: "许可边界".to_string(),
            status: "warn".to_string(),
            evidence: asset.license.clone().if_empty("许可证尚未确认"),
        });
        suggestions.push("补录许可证或内部使用边界，避免误发布和错误复用。".to_string());
    } else {
        score += 10;
        checks.push(AuditCheck {
            name: "许可边界".to_string(),
            status: "pass".to_string(),
            evidence: asset.license.clone(),
        });
    }

    if asset.ready_versions.is_empty() {
        checks.push(AuditCheck {
            name: "复用证据".to_string(),
            status: "warn".to_string(),
            evidence: "尚无固定版本与验证回执".to_string(),
        });
        suggestions
            .push("在目标工具中完成一次真实加载或最小任务验证，再登记固定版本。".to_string());
    } else {
        score += 5;
        checks.push(AuditCheck {
            name: "复用证据".to_string(),
            status: "pass".to_string(),
            evidence: format!("已有 {} 个就绪版本", asset.ready_versions.len()),
        });
    }

    let score = score.clamp(0, 100) as u8;
    // A readable legacy definition without current frontmatter is actionable, not unavailable.
    // Keep it in the governance queue instead of mislabeling third-party definitions as blocked.
    let status = if visible == 0 {
        "blocked"
    } else if score >= 80 && variant_count == 1 && (!deep || hash_matches == variant_count) {
        "usable"
    } else {
        "needs_attention"
    };
    AssetAuditReport {
        logical_id: asset.logical_id.clone(),
        name: asset.name.clone(),
        kind: asset.kind.clone(),
        status: status.to_string(),
        score,
        checks,
        suggestions,
    }
}

trait EmptyFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.trim().is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

fn run_audit_blocking(logical_ids: Vec<String>, deep: bool) -> Result<AssetAuditResult, String> {
    let started = Instant::now();
    let registry = read_registry()?;
    let scope = if logical_ids.is_empty() {
        "all"
    } else {
        "selected"
    };
    let selected: std::collections::HashSet<_> = logical_ids.into_iter().collect();
    let mut reports = registry
        .groups
        .iter()
        .filter(|asset| {
            asset.kind.eq_ignore_ascii_case("agent") || asset.kind.eq_ignore_ascii_case("skill")
        })
        .filter(|asset| selected.is_empty() || selected.contains(&asset.logical_id))
        .map(|asset| audit_asset(asset, deep))
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| {
        left.score
            .cmp(&right.score)
            .then_with(|| left.name.cmp(&right.name))
    });
    let total = reports.len();
    let usable = reports
        .iter()
        .filter(|report| report.status == "usable")
        .count();
    let needs_attention = reports
        .iter()
        .filter(|report| report.status == "needs_attention")
        .count();
    let blocked = reports
        .iter()
        .filter(|report| report.status == "blocked")
        .count();
    let truncated = reports.len() > MAX_RETURNED_REPORTS;
    reports.truncate(MAX_RETURNED_REPORTS);
    Ok(AssetAuditResult {
        scope: scope.to_string(),
        truth_level: if deep { "static_deep" } else { "static_light" }.to_string(),
        total,
        usable,
        needs_attention,
        blocked,
        duration_ms: started.elapsed().as_millis(),
        checked_at: Local::now().to_rfc3339(),
        reports,
        truncated,
    })
}

#[tauri::command]
pub async fn run_asset_audit(
    logical_ids: Vec<String>,
    deep: bool,
) -> Result<AssetAuditResult, String> {
    tauri::async_runtime::spawn_blocking(move || run_audit_blocking(logical_ids, deep))
        .await
        .map_err(|error| format!("资产验真任务异常终止：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Location, Variant};

    fn sample(path: &Path, kind: &str) -> AssetGroup {
        let bytes = fs::read(path).unwrap();
        AssetGroup {
            logical_id: "sample".to_string(),
            kind: kind.to_string(),
            name: "Sample".to_string(),
            license: "MIT".to_string(),
            variants: vec![Variant {
                sha256: format!("{:x}", Sha256::digest(bytes)),
                locations: vec![Location {
                    path: path.display().to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn drifted_copy_cannot_be_hidden_by_location_order() {
        let root =
            std::env::temp_dir().join(format!("lingzhan-audit-order-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let valid = root.join("valid.md");
        let drifted = root.join("drifted.md");
        fs::write(&valid, "name: sample\ndescription: test\n").unwrap();
        fs::write(&drifted, "changed").unwrap();
        let mut asset = sample(&valid, "Skill");
        asset.variants[0].locations.push(Location {
            path: drifted.display().to_string(),
            ..Default::default()
        });
        let before = inspect_definition(&asset, true);
        asset.variants[0].locations.reverse();
        assert_eq!(before, inspect_definition(&asset, true));
        assert_eq!(before, (1, 1, 0));
        assert!(audit_asset(&asset, true)
            .checks
            .iter()
            .any(|c| c.name == "内容哈希" && c.status != "pass"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deep_audit_distinguishes_valid_and_missing_sources() {
        let path = std::env::temp_dir().join(format!("lingzhan-audit-{}.md", std::process::id()));
        fs::write(&path, "---\nname: sample\ndescription: test\n---\n").unwrap();
        let valid = audit_asset(&sample(&path, "Skill"), true);
        assert_ne!(valid.status, "blocked");
        let missing = audit_asset(&sample(&path, "Skill"), true);
        fs::remove_file(&path).unwrap();
        let blocked = audit_asset(&sample_without_read(&missing), true);
        assert_eq!(blocked.status, "blocked");
    }

    #[test]
    fn readable_legacy_skill_needs_attention_instead_of_being_blocked() {
        let path =
            std::env::temp_dir().join(format!("lingzhan-legacy-skill-{}.md", std::process::id()));
        fs::write(
            &path,
            "# Legacy Skill\n\nReadable guidance without frontmatter.\n",
        )
        .unwrap();
        let report = audit_asset(&sample(&path, "Skill"), true);
        assert_eq!(report.status, "needs_attention");
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "结构完整" && check.status == "fail"));
        fs::remove_file(path).unwrap();
    }

    fn sample_without_read(report: &AssetAuditReport) -> AssetGroup {
        AssetGroup {
            logical_id: report.logical_id.clone(),
            kind: report.kind.clone(),
            name: report.name.clone(),
            license: "MIT".to_string(),
            variants: vec![crate::types::Variant {
                sha256: "missing".to_string(),
                locations: vec![crate::types::Location {
                    path: "/definitely/missing/lingzhan".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }
}
