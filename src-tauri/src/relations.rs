use crate::registry::read_registry;
use crate::types::{AgentSkillResolution, AssetGroup, CompanionSkill, Registry, RelationEvidence};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_RELATION_FILE_BYTES: u64 = 1024 * 1024;
const MAX_RELATION_FILES: usize = 12;

fn is_agent(group: &AssetGroup) -> bool {
    group.kind.eq_ignore_ascii_case("agent")
}

fn is_skill(group: &AssetGroup) -> bool {
    group.kind.eq_ignore_ascii_case("skill")
}

fn normalize_alias(value: &str) -> String {
    value.trim().to_lowercase()
}

fn normalize_key(value: &str) -> String {
    value
        .trim()
        .trim_matches(|ch| matches!(ch, '"' | '\'' | '`'))
        .to_lowercase()
        .replace(['-', ' '], "_")
}

fn is_relation_key(value: &str) -> bool {
    matches!(
        normalize_key(value).as_str(),
        "primary_skill"
            | "skills"
            | "related_skills"
            | "skill_ids"
            | "uses_skills"
            | "required_skills"
    )
}

fn is_skill_heading(line: &str) -> bool {
    let heading = line
        .trim_start_matches('#')
        .trim()
        .to_lowercase()
        .replace(' ', "_");
    matches!(
        heading.as_str(),
        "skills"
            | "available_skills"
            | "related_skills"
            | "required_skills"
            | "required_workflow_skills"
            | "拥有的_skills"
            | "可用_skills"
            | "关联_skills"
            | "需要的_skills"
    )
}

fn clean_reference(value: &str) -> Option<String> {
    let without_comment = value.split('#').next().unwrap_or_default();
    let trimmed = without_comment
        .trim()
        .trim_start_matches('-')
        .trim()
        .trim_matches(|ch| {
            matches!(
                ch,
                '"' | '\'' | '`' | '[' | ']' | '(' | ')' | '{' | '}' | '*'
            )
        })
        .trim_end_matches(['.', ';', ':'])
        .trim();
    if trimmed.len() < 2 || trimmed.len() > 128 || trimmed.chars().any(char::is_whitespace) {
        return None;
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '@'))
    {
        return None;
    }
    Some(trimmed.to_string())
}

fn references_from_value(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter_map(clean_reference)
        .collect::<Vec<_>>()
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let colon = trimmed.find(':');
    let equals = trimmed.find('=');
    let separator = match (colon, equals) {
        (Some(left), Some(right)) => left.min(right),
        (Some(index), None) | (None, Some(index)) => index,
        (None, None) => return None,
    };
    Some((&trimmed[..separator], &trimmed[separator + 1..]))
}

fn extract_explicit_references(content: &str) -> Vec<String> {
    let mut references = Vec::new();
    let mut list_active = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            list_active = is_skill_heading(trimmed);
            continue;
        }

        if let Some((key, value)) = split_key_value(trimmed) {
            if is_relation_key(key) {
                references.extend(references_from_value(value));
                list_active = value.trim().is_empty() || matches!(value.trim(), "[]" | "[");
                continue;
            }
            if !trimmed.starts_with('-') {
                list_active = false;
            }
        }

        if list_active {
            if trimmed.starts_with('-') {
                if let Some(reference) = clean_reference(trimmed) {
                    references.push(reference);
                }
            } else if !trimmed.is_empty() {
                list_active = false;
            }
        }
    }

    let mut seen = HashSet::new();
    references.retain(|reference| seen.insert(normalize_alias(reference)));
    references
}

fn skill_aliases(group: &AssetGroup) -> BTreeSet<String> {
    let mut aliases = BTreeSet::from([
        normalize_alias(&group.logical_id),
        normalize_alias(&group.name),
    ]);
    for variant in &group.variants {
        for location in &variant.locations {
            let path = Path::new(&location.path);
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("SKILL.md"))
            {
                if let Some(parent_name) = path.parent().and_then(Path::file_name) {
                    aliases.insert(normalize_alias(&parent_name.to_string_lossy()));
                }
            }
        }
    }
    aliases.retain(|alias| !alias.is_empty());
    aliases
}

fn build_alias_index(registry: &Registry) -> HashMap<String, Vec<usize>> {
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();
    for (group_index, group) in registry.groups.iter().enumerate() {
        if !is_skill(group) {
            continue;
        }
        for alias in skill_aliases(group) {
            index.entry(alias).or_default().push(group_index);
        }
    }
    index
}

fn relation_sources(agent: &AssetGroup) -> (Vec<PathBuf>, Vec<String>) {
    let mut paths = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();

    'variants: for variant in &agent.variants {
        for location in &variant.locations {
            if paths.len() >= MAX_RELATION_FILES {
                warnings.push(format!(
                    "关系分析最多读取 {MAX_RELATION_FILES} 个已登记 Agent 定义文件"
                ));
                break 'variants;
            }
            let source = Path::new(&location.path);
            let canonical = match fs::canonicalize(source) {
                Ok(path) => path,
                Err(_) => {
                    warnings.push(format!("Agent 定义不可访问：{}", source.display()));
                    continue;
                }
            };
            if !canonical.is_file() || !seen.insert(canonical.clone()) {
                continue;
            }
            let allowed = canonical
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "md" | "toml" | "yaml" | "yml" | "json"
                    )
                });
            if !allowed {
                warnings.push(format!(
                    "关系分析跳过不支持的定义格式：{}",
                    canonical.display()
                ));
                continue;
            }
            match fs::metadata(&canonical) {
                Ok(metadata) if metadata.len() <= MAX_RELATION_FILE_BYTES => {
                    paths.push(canonical);
                }
                Ok(_) => warnings.push(format!(
                    "关系分析跳过超过 1 MiB 的定义：{}",
                    canonical.display()
                )),
                Err(_) => warnings.push(format!("无法读取定义元数据：{}", canonical.display())),
            }
        }
    }
    (paths, warnings)
}

pub(crate) fn resolve_agent_skills_from_registry(
    registry: &Registry,
    agent_logical_id: &str,
) -> Result<AgentSkillResolution, String> {
    let agent = registry
        .groups
        .iter()
        .find(|group| group.logical_id == agent_logical_id)
        .ok_or_else(|| "Agent 已不在当前注册表；请刷新后重试".to_string())?;
    if !is_agent(agent) {
        return Err("配合 Skill 分析只接受 Agent".to_string());
    }

    let alias_index = build_alias_index(registry);
    let (sources, mut warnings) = relation_sources(agent);
    let mut inspected_paths = Vec::new();
    let mut unresolved = BTreeSet::new();
    let mut resolved: BTreeMap<String, CompanionSkill> = BTreeMap::new();

    for source in sources {
        let content = match fs::read_to_string(&source) {
            Ok(value) => value,
            Err(_) => {
                warnings.push(format!("Agent 定义不是可读 UTF-8：{}", source.display()));
                continue;
            }
        };
        inspected_paths.push(source.display().to_string());
        for reference in extract_explicit_references(&content) {
            let alias = normalize_alias(&reference);
            let matches = alias_index.get(&alias).cloned().unwrap_or_default();
            if matches.len() != 1 {
                unresolved.insert(reference);
                if matches.len() > 1 {
                    warnings.push(format!("配合 Skill 引用存在多个逻辑匹配：{alias}"));
                }
                continue;
            }
            let skill = &registry.groups[matches[0]];
            let entry =
                resolved
                    .entry(skill.logical_id.clone())
                    .or_insert_with(|| CompanionSkill {
                        logical_id: skill.logical_id.clone(),
                        name: skill.name.clone(),
                        evidence: Vec::new(),
                    });
            entry.evidence.push(RelationEvidence {
                reference,
                source_path: source.display().to_string(),
                rule: "explicit_agent_definition".to_string(),
            });
        }
    }

    Ok(AgentSkillResolution {
        agent_logical_id: agent.logical_id.clone(),
        agent_name: agent.name.clone(),
        skills: resolved.into_values().collect(),
        unresolved_references: unresolved.into_iter().collect(),
        inspected_paths,
        warnings,
    })
}

fn resolve_agent_skills_blocking(agent_logical_id: String) -> Result<AgentSkillResolution, String> {
    let registry = read_registry()?;
    resolve_agent_skills_from_registry(&registry, &agent_logical_id)
}

#[tauri::command]
pub async fn resolve_agent_skills(
    agent_logical_id: String,
) -> Result<AgentSkillResolution, String> {
    tauri::async_runtime::spawn_blocking(move || resolve_agent_skills_blocking(agent_logical_id))
        .await
        .map_err(|error| format!("关系分析任务异常终止：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Location, Variant};

    fn group(logical_id: &str, kind: &str, name: &str, path: &Path) -> AssetGroup {
        AssetGroup {
            logical_id: logical_id.to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            variants: vec![Variant {
                sha256: "fixture".to_string(),
                locations: vec![Location {
                    path: path.display().to_string(),
                    ..Location::default()
                }],
                ..Variant::default()
            }],
            ..AssetGroup::default()
        }
    }

    #[test]
    fn explicit_agent_fields_resolve_companion_skills() {
        let root = std::env::temp_dir().join(format!(
            "agent-skill-hub-relations-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("skill-one")).expect("create fixture");
        fs::create_dir_all(root.join("skill-two")).expect("create fixture");
        let agent_path = root.join("agent.md");
        fs::write(
            &agent_path,
            "---\nprimary_skill: skill-one\n---\n## Available Skills\n- `skill-two`\n## Rules\n- unrelated-skill\n",
        )
        .expect("write agent");
        let skill_one = root.join("skill-one/SKILL.md");
        let skill_two = root.join("skill-two/SKILL.md");
        fs::write(&skill_one, "# one").expect("write skill");
        fs::write(&skill_two, "# two").expect("write skill");

        let registry = Registry {
            schema_version: 1,
            physical_entry_count: 3,
            logical_asset_count: 3,
            unique_hash_count: 3,
            updated_at: "fixture".to_string(),
            groups: vec![
                group("agent-one", "Agent", "Agent One", &agent_path),
                group("auto-skill-one", "Skill", "skill-one", &skill_one),
                group("auto-skill-two", "Skill", "skill-two", &skill_two),
            ],
            ..Registry::default()
        };

        let resolution =
            resolve_agent_skills_from_registry(&registry, "agent-one").expect("resolve relations");
        assert_eq!(resolution.skills.len(), 2);
        assert!(resolution
            .skills
            .iter()
            .any(|skill| skill.logical_id == "auto-skill-one"));
        assert!(resolution
            .skills
            .iter()
            .any(|skill| skill.logical_id == "auto-skill-two"));
        assert!(!resolution
            .unresolved_references
            .contains(&"unrelated-skill".to_string()));

        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn ambiguous_alias_is_not_silently_selected() {
        let root = std::env::temp_dir().join(format!(
            "agent-skill-hub-relations-ambiguous-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create fixture");
        let agent_path = root.join("agent.md");
        fs::write(&agent_path, "primary_skill: duplicate").expect("write agent");
        let registry = Registry {
            schema_version: 1,
            physical_entry_count: 3,
            logical_asset_count: 3,
            unique_hash_count: 3,
            updated_at: "fixture".to_string(),
            groups: vec![
                group("agent-one", "Agent", "Agent One", &agent_path),
                group("skill-a", "Skill", "duplicate", &root.join("one.md")),
                group("skill-b", "Skill", "duplicate", &root.join("two.md")),
            ],
            ..Registry::default()
        };

        let resolution =
            resolve_agent_skills_from_registry(&registry, "agent-one").expect("resolve relations");
        assert!(resolution.skills.is_empty());
        assert_eq!(resolution.unresolved_references, vec!["duplicate"]);
        assert!(resolution
            .warnings
            .iter()
            .any(|warning| warning.contains("多个逻辑匹配")));

        fs::remove_dir_all(root).expect("remove fixture");
    }
}
