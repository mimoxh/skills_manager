use crate::{
    error::AppResult,
    manifest::parse_skill_frontmatter,
    models::{AgentProfile, AgentSkillCopy, AgentType, GroupedSkill},
    util::{normalize_title, system_time_to_rfc3339},
};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

/// 扫描单个 Agent 的 skills 目录，返回每个子目录对应的 AgentSkillCopy。
pub(crate) fn scan_agent_skill_copies(agent: &AgentProfile) -> AppResult<Vec<AgentSkillCopy>> {
    let root = Path::new(&agent.skills_path);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let registered_skill_ids = if agent.agent_type == AgentType::ClaudeCowork {
        Some(read_claude_cowork_registered_skill_ids(agent)?)
    } else {
        None
    };
    let mut copies = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name();
        if dir_name.to_string_lossy().starts_with('.') {
            continue;
        }
        let dir_id = dir_name.to_string_lossy().to_string();
        let is_registered = registered_skill_ids
            .as_ref()
            .map(|ids| ids.contains(&dir_id))
            .unwrap_or(true);
        let metadata = fs::metadata(&path).ok();
        let (title, version, description, readme) = read_agent_skill_info(&path, false);
        copies.push(AgentSkillCopy {
            agent_id: agent.id.clone(),
            agent_name: agent.name.clone(),
            skill_path: path.to_string_lossy().to_string(),
            title,
            version,
            updated_at: metadata
                .and_then(|metadata| metadata.modified().ok())
                .map(system_time_to_rfc3339),
            description,
            readme,
            is_registered,
        });
    }
    Ok(copies)
}

/// 将多个 Agent 的 skill copies 按标题分组，挑选最高版本作为 best_copy。
pub(crate) fn group_agent_skills(agents: &[AgentProfile], copies: Vec<AgentSkillCopy>) -> Vec<GroupedSkill> {
    let mut grouped: HashMap<String, Vec<AgentSkillCopy>> = HashMap::new();
    for copy in copies {
        grouped
            .entry(normalize_title(&copy.title))
            .or_default()
            .push(copy);
    }

    let mut values = grouped
        .into_values()
        .map(|mut copies| {
            copies.sort_by(compare_skill_copy);
            let best_copy = copies[0].clone();
            let installed_set = copies
                .iter()
                .map(|copy| copy.agent_id.clone())
                .collect::<HashSet<_>>();
            let missing_agent_ids = agents
                .iter()
                .filter(|agent| !installed_set.contains(&agent.id))
                .map(|agent| agent.id.clone())
                .collect::<Vec<_>>();
            let mut installed_agent_ids = copies
                .iter()
                .map(|copy| copy.agent_id.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            installed_agent_ids.sort();
            GroupedSkill {
                title: best_copy.title.clone(),
                description: best_copy.description.clone(),
                readme: best_copy.readme.clone(),
                user_tags: Vec::new(),
                best_copy,
                copies,
                installed_agent_ids,
                missing_agent_ids,
            }
        })
        .collect::<Vec<_>>();
    values.sort_by(|a, b| a.title.cmp(&b.title));
    values
}

/// Check if a description contains meaningful text (not just symbols/punctuation).
pub(crate) fn is_valid_description(desc: &str) -> bool {
    desc.chars().any(|c| {
        c.is_alphanumeric() || c.is_ascii_alphabetic() || ('\u{4e00}'..='\u{9fff}').contains(&c)
    })
}

/// 读取技能目录元数据：优先 skill.json/skill.yaml/skill.yml manifest，
/// 其次 SKILL.md frontmatter，最后目录名。返回 (title, version, description, readme)。
pub(crate) fn read_agent_skill_info(
    skill_path: &Path,
    include_readme: bool,
) -> (String, Option<String>, Option<String>, Option<String>) {
    for name in ["skill.json", "skill.yaml", "skill.yml"] {
        let manifest_path = skill_path.join(name);
        if !manifest_path.exists() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&manifest_path) {
            let parsed = match manifest_path.extension().and_then(|value| value.to_str()) {
                Some("json") => serde_json::from_str::<serde_json::Value>(&text).ok(),
                _ => serde_yaml::from_str::<serde_json::Value>(&text).ok(),
            };
            if let Some(value) = parsed {
                let title = value
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                let version = value
                    .get("version")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string);
                let description = value
                    .get("description")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty() && is_valid_description(value))
                    .map(ToString::to_string);
                if let Some(title) = title {
                    let readme = include_readme
                        .then(|| read_agent_skill_readme(skill_path))
                        .flatten();
                    let description = description.or_else(|| {
                        fs::read_to_string(skill_path.join("SKILL.md"))
                            .ok()
                            .and_then(|text| read_markdown_frontmatter(&text))
                            .and_then(|(_title, _version, description)| description)
                    });
                    return (title.to_string(), version, description, readme);
                }
            }
        }
    }

    let skill_md = skill_path.join("SKILL.md");
    if let Ok(text) = fs::read_to_string(&skill_md) {
        let readme = include_readme
            .then(|| extract_markdown_body(&text))
            .flatten();
        if let Some((title, version, description)) = read_markdown_frontmatter(&text) {
            return (title, version, description, readme);
        }
        if let Some(title) = read_markdown_heading(&text) {
            return (title, None, None, readme);
        }
    }

    (
        skill_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Untitled Skill")
            .to_string(),
        None,
        None,
        None,
    )
}

/// 读取技能目录中的 SKILL.md 正文（去除 frontmatter）。
pub(crate) fn read_agent_skill_readme(skill_path: &Path) -> Option<String> {
    fs::read_to_string(skill_path.join("SKILL.md"))
        .ok()
        .and_then(|text| extract_markdown_body(&text))
}

pub(crate) fn extract_markdown_body(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.starts_with("---") {
        return Some(trimmed.to_string());
    }
    let after_first = &trimmed[3..];
    let Some(end_idx) = after_first.find("\n---") else {
        return Some(trimmed.to_string());
    };
    let body = after_first[end_idx + 4..].trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
}

pub(crate) fn read_markdown_frontmatter(text: &str) -> Option<(String, Option<String>, Option<String>)> {
    let fm = parse_skill_frontmatter(text)?;
    let title = fm.name.or_else(|| read_markdown_heading(text))?;
    let description = fm.description.filter(|desc| is_valid_description(desc));
    Some((title, fm.version, description))
}

fn read_markdown_heading(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("# "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn compare_skill_copy(a: &AgentSkillCopy, b: &AgentSkillCopy) -> Ordering {
    compare_versions(b.version.as_deref(), a.version.as_deref())
        .then_with(|| b.updated_at.cmp(&a.updated_at))
        .then_with(|| a.agent_name.cmp(&b.agent_name))
        .then_with(|| a.skill_path.cmp(&b.skill_path))
}

pub(crate) fn compare_versions(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (parse_version(a), parse_version(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn parse_version(version: Option<&str>) -> Option<Vec<u64>> {
    let version = version?.trim().trim_start_matches('v');
    if version.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for part in version.split('.') {
        let digits = part
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if digits.is_empty() {
            return None;
        }
        parts.push(digits.parse().ok()?);
    }
    Some(parts)
}

// ---- Claude Cowork manifest 读写（从 service.rs 搬入，与 skill_scan 合并避免循环依赖）----

/// 定位 Claude Cowork manifest.json 路径。
pub(crate) fn claude_cowork_manifest_path(agent: &AgentProfile) -> AppResult<PathBuf> {
    if agent.agent_type != AgentType::ClaudeCowork {
        return Err(crate::error::AppError::Message(
            "Agent 不是 Claude Desktop Cowork".to_string(),
        ));
    }
    if let Some(path) = agent
        .adapter_config
        .as_ref()
        .and_then(|value| value.get("manifestPath"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(PathBuf::from(path));
    }
    Path::new(&agent.skills_path)
        .parent()
        .map(|parent| parent.join("manifest.json"))
        .ok_or_else(|| {
            crate::error::AppError::Message("无法确定 Cowork manifest 路径".to_string())
        })
}

pub(crate) fn read_claude_cowork_manifest(agent: &AgentProfile) -> AppResult<serde_json::Value> {
    let manifest_path = claude_cowork_manifest_path(agent)?;
    if !manifest_path.exists() {
        return Ok(serde_json::json!({ "skills": [] }));
    }
    let value = serde_json::from_str::<serde_json::Value>(&fs::read_to_string(manifest_path)?)?;
    Ok(if value.is_object() {
        value
    } else {
        serde_json::json!({ "skills": [] })
    })
}

pub(crate) fn read_claude_cowork_registered_skill_ids(agent: &AgentProfile) -> AppResult<HashSet<String>> {
    let manifest = read_claude_cowork_manifest(agent)?;
    Ok(manifest
        .get("skills")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|skill| skill.get("skillId").and_then(|value| value.as_str()))
        .map(ToString::to_string)
        .collect())
}

pub(crate) fn register_claude_cowork_skill(
    agent: &AgentProfile,
    skill_id: &str,
    skill_path: &Path,
) -> AppResult<()> {
    let manifest_path = claude_cowork_manifest_path(agent)?;
    let mut manifest = read_claude_cowork_manifest(agent)?;
    if !manifest.is_object() {
        manifest = serde_json::json!({});
    }
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| crate::error::AppError::Message("Cowork manifest 必须是 JSON object".to_string()))?;
    object.insert(
        "lastUpdated".to_string(),
        serde_json::Value::Number(chrono::Utc::now().timestamp_millis().into()),
    );

    let (name, _version, description, _readme) = read_agent_skill_info(skill_path, false);
    let skills_value = object
        .entry("skills".to_string())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    if !skills_value.is_array() {
        *skills_value = serde_json::Value::Array(Vec::new());
    }
    let skills = skills_value
        .as_array_mut()
        .ok_or_else(|| crate::error::AppError::Message("Cowork manifest skills 必须是数组".to_string()))?;
    let now = chrono::Utc::now().to_rfc3339();
    let existing_index = skills.iter().position(|entry| {
        entry
            .get("skillId")
            .and_then(|value| value.as_str())
            .map(|value| value == skill_id)
            .unwrap_or(false)
    });
    let entry_index = if let Some(index) = existing_index {
        index
    } else {
        skills.push(serde_json::json!({}));
        skills.len() - 1
    };
    let entry = &mut skills[entry_index];
    let entry_object = entry
        .as_object_mut()
        .ok_or_else(|| crate::error::AppError::Message("Cowork manifest skill 条目必须是 object".to_string()))?;
    entry_object.insert(
        "skillId".to_string(),
        serde_json::Value::String(skill_id.to_string()),
    );
    entry_object.insert("name".to_string(), serde_json::Value::String(name));
    if let Some(description) = description {
        entry_object.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    entry_object.insert(
        "creatorType".to_string(),
        serde_json::Value::String("user".to_string()),
    );
    entry_object.insert("syncManaged".to_string(), serde_json::Value::Bool(false));
    entry_object.insert("updatedAt".to_string(), serde_json::Value::String(now));
    entry_object.insert("enabled".to_string(), serde_json::Value::Bool(true));

    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_agent_skill(
        root: &Path,
        dir: &str,
        manifest_name: Option<&str>,
        version: Option<&str>,
        markdown: &str,
    ) {
        let skill_dir = root.join(dir);
        fs::create_dir_all(&skill_dir).unwrap();
        if manifest_name.is_some() || version.is_some() {
            let manifest = serde_json::json!({
                "id": dir,
                "name": manifest_name.unwrap_or(dir),
                "version": version.unwrap_or("1.0.0"),
                "supportedAgents": ["*"],
                "files": ["SKILL.md"]
            });
            fs::write(
                skill_dir.join("skill.json"),
                serde_json::to_string(&manifest).unwrap(),
            )
            .unwrap();
        }
        fs::write(skill_dir.join("SKILL.md"), markdown).unwrap();
    }

    #[test]
    fn reads_agent_skill_title_by_manifest_frontmatter_heading_then_dir() {
        let root = tempfile::tempdir().unwrap();
        // 1. manifest name 优先
        write_agent_skill(root.path(), "a", Some("ManifestA"), Some("2.0.0"), "# HeadingA");
        // 2. SKILL.md frontmatter
        write_agent_skill(root.path(), "b", None, None, "---\nname: FmB\nversion: 1.0.0\n---\nbody");
        // 3. SKILL.md 标题
        write_agent_skill(root.path(), "c", None, None, "# HeadingC");
        // 4. 目录名兜底
        write_agent_skill(root.path(), "d", None, None, "no title");

        let agent = AgentProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            agent_type: AgentType::Custom,
            skills_path: root.path().to_string_lossy().to_string(),
            adapter_config: None,
            user_tags: Vec::new(),
        };
        let copies = scan_agent_skill_copies(&agent).unwrap();
        let by_dir: HashMap<_, _> = copies
            .iter()
            .map(|c| {
                (
                    Path::new(&c.skill_path).file_name().unwrap().to_str().unwrap().to_string(),
                    c.title.clone(),
                )
            })
            .collect();
        assert_eq!(by_dir.get("a"), Some(&"ManifestA".to_string()));
        assert_eq!(by_dir.get("b"), Some(&"FmB".to_string()));
        assert_eq!(by_dir.get("c"), Some(&"HeadingC".to_string()));
        assert_eq!(by_dir.get("d"), Some(&"d".to_string()));
    }

    #[test]
    fn reads_yaml_block_scalar_description() {
        let root = tempfile::tempdir().unwrap();
        let skill_dir = root.path().join("skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("skill.yaml"),
            r#"name: YAML Skill
version: 1.0.0
description: |
  多行描述
  第二行
"#,
        )
        .unwrap();
        let (title, version, description, _readme) = read_agent_skill_info(&skill_dir, false);
        assert_eq!(title, "YAML Skill");
        assert_eq!(version.as_deref(), Some("1.0.0"));
        assert!(description.unwrap().contains("第二行"));
    }

    #[test]
    fn groups_agent_skills_and_picks_highest_version() {
        let root = tempfile::tempdir().unwrap();
        let a1 = root.path().join("agent1");
        let a2 = root.path().join("agent2");
        fs::create_dir_all(&a1).unwrap();
        fs::create_dir_all(&a2).unwrap();
        write_agent_skill(&a1, "demo", Some("Demo Skill"), Some("1.0.0"), "# Demo");
        write_agent_skill(&a2, "demo", Some("Demo Skill"), Some("2.0.0"), "# Demo");

        let agents = vec![
            AgentProfile {
                id: "agent1".to_string(),
                name: "Agent 1".to_string(),
                agent_type: AgentType::Custom,
                skills_path: a1.to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
            },
            AgentProfile {
                id: "agent2".to_string(),
                name: "Agent 2".to_string(),
                agent_type: AgentType::Custom,
                skills_path: a2.to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
            },
        ];
        let mut copies = Vec::new();
        for agent in &agents {
            copies.extend(scan_agent_skill_copies(agent).unwrap());
        }
        let groups = group_agent_skills(&agents, copies);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].best_copy.version.as_deref(), Some("2.0.0"));
        assert_eq!(groups[0].installed_agent_ids.len(), 2);
        assert!(groups[0].missing_agent_ids.is_empty());
    }

    #[test]
    fn skips_hidden_directories_when_scanning_agent_skills() {
        let root = tempfile::tempdir().unwrap();
        let skill_dir = root.path().join("skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::create_dir_all(root.path().join(".hidden")).unwrap();
        write_agent_skill(root.path(), "skill", Some("Visible"), Some("1.0.0"), "# Visible");

        let agent = AgentProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            agent_type: AgentType::Custom,
            skills_path: root.path().to_string_lossy().to_string(),
            adapter_config: None,
            user_tags: Vec::new(),
        };
        let copies = scan_agent_skill_copies(&agent).unwrap();
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].title, "Visible");
    }

    #[test]
    fn compares_versions_with_prerelease_and_v_prefix() {
        use std::cmp::Ordering;
        // 预发布后缀（1.0.0-beta）按主版本取前导数字比较，不因后缀判 None
        assert_eq!(compare_versions(Some("1.0.0-beta"), Some("1.0.0")), Ordering::Equal);
        assert_eq!(compare_versions(Some("2.0.0-beta"), Some("1.9.9")), Ordering::Greater);
        // v 前缀忽略
        assert_eq!(compare_versions(Some("v1.2.3"), Some("1.2.3")), Ordering::Equal);
        // 版本缺失排序到末尾
        assert_eq!(compare_versions(Some("1.0.0"), None), Ordering::Greater);
        assert_eq!(compare_versions(None, Some("1.0.0")), Ordering::Less);
    }
}
