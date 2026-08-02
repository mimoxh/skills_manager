use crate::{
    error::{AppError, AppResult},
    manifest::{read_markdown_heading, read_skill_md_frontmatter},
    models::{CatalogInstallStatus, CatalogSkill, CatalogSort, CatalogSource},
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{cmp::Ordering, fs, path::Path};
use walkdir::WalkDir;

pub const CLAWHUB_API_CACHE_FILE: &str = "clawhub-skills.json";

pub fn scan_catalog_repository(
    repository: &Path,
    source: &CatalogSource,
) -> AppResult<Vec<CatalogSkill>> {
    if !repository.exists() {
        return Err(AppError::Message(format!(
            "Catalog repository does not exist: {}",
            repository.display()
        )));
    }

    let mut skills = Vec::new();
    for entry in WalkDir::new(repository)
        .max_depth(6)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name != "skill.md" && name != "skill.json" && name != "skill.yaml" && name != "skill.yml"
        {
            continue;
        }
        let Some(skill_dir) = entry.path().parent() else {
            continue;
        };
        if skills
            .iter()
            .any(|skill: &CatalogSkill| Path::new(&skill.source_path) == skill_dir)
        {
            continue;
        }
        skills.push(read_catalog_skill(repository, skill_dir, source)?);
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

pub fn scan_clawhub_api_cache(
    cache_path: &Path,
    source: &CatalogSource,
) -> AppResult<Vec<CatalogSkill>> {
    let cache_file = cache_path.join(CLAWHUB_API_CACHE_FILE);
    if !cache_file.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(cache_file)?;
    parse_clawhub_api_catalog(&text, source)
}

pub fn parse_clawhub_api_catalog(
    text: &str,
    source: &CatalogSource,
) -> AppResult<Vec<CatalogSkill>> {
    let value = serde_json::from_str::<Value>(text)?;
    let items = value
        .get("items")
        .or_else(|| value.get("skills"))
        .or_else(|| value.get("results"))
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| AppError::Message("ClawHub API 响应中没有 skills 列表。".to_string()))?;
    let mut skills = items
        .iter()
        .filter_map(|item| clawhub_item_to_skill(item, source))
        .collect::<Vec<_>>();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

pub fn sort_catalog_skills(mut skills: Vec<CatalogSkill>, sort: CatalogSort) -> Vec<CatalogSkill> {
    skills.sort_by(|a, b| match sort {
        CatalogSort::Downloads => compare_option_desc(
            a.download_count.or(a.install_count),
            b.download_count.or(b.install_count),
        )
        .then_with(|| a.name.cmp(&b.name)),
        CatalogSort::PublishedDesc => compare_option_text_desc(&a.published_at, &b.published_at)
            .then_with(|| a.name.cmp(&b.name)),
        CatalogSort::UpdatedDesc => {
            compare_option_text_desc(&a.updated_at, &b.updated_at).then_with(|| a.name.cmp(&b.name))
        }
        CatalogSort::Source => source_rank(&a.source_id)
            .cmp(&source_rank(&b.source_id))
            .then_with(|| a.source_name.cmp(&b.source_name))
            .then_with(|| a.name.cmp(&b.name)),
    });
    skills
}

fn read_catalog_skill(
    repository: &Path,
    skill_dir: &Path,
    source: &CatalogSource,
) -> AppResult<CatalogSkill> {
    let skill_md = skill_dir.join("SKILL.md");
    let manifest = read_manifest_value(skill_dir);
    let skill_md_text = fs::read_to_string(&skill_md).ok();
    let (frontmatter_name, frontmatter_description, frontmatter_tags) = skill_md_text
        .as_deref()
        .and_then(read_skill_md_frontmatter)
        .unwrap_or((None, None, Vec::new()));
    let title = manifest
        .as_ref()
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or(frontmatter_name)
        .or_else(|| read_markdown_heading(skill_md_text.as_deref().unwrap_or("")))
        .or_else(|| {
            skill_dir
                .file_name()
                .and_then(|value| value.to_str())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "Untitled Skill".to_string());
    let description = manifest
        .as_ref()
        .and_then(|value| value.get("description"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or(frontmatter_description);
    let tags = manifest
        .as_ref()
        .and_then(|value| value.get("tags"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or(frontmatter_tags);
    let supported_agents = manifest
        .as_ref()
        .and_then(|value| {
            value
                .get("supportedAgents")
                .or_else(|| value.get("supported_agents"))
        })
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| infer_supported_agents(source));
    let relative_path = skill_dir
        .strip_prefix(repository)
        .unwrap_or(skill_dir)
        .to_string_lossy()
        .replace('\\', "/");
    let updated_at = fs::metadata(skill_dir)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(|time| DateTime::<Utc>::from(time).to_rfc3339());

    Ok(CatalogSkill {
        id: format!("{}::{}", source.id, relative_path),
        name: title,
        source_id: source.id.clone(),
        source_name: source.name.clone(),
        source_icon: source.icon.clone(),
        source_path: skill_dir.to_string_lossy().to_string(),
        relative_path,
        description,
        tags,
        supported_agents,
        published_at: None,
        updated_at,
        download_count: None,
        install_count: None,
        has_skill_md: skill_md.exists(),
        has_scripts: skill_dir.join("scripts").is_dir(),
        has_references: skill_dir.join("references").is_dir(),
        has_assets: skill_dir.join("assets").is_dir(),
        install_status: CatalogInstallStatus::NotInstalled,
    })
}

fn clawhub_item_to_skill(item: &Value, source: &CatalogSource) -> Option<CatalogSkill> {
    let slug = read_text(item, &["slug", "name", "id"])?;
    let title = read_text(item, &["displayName", "display_name", "title", "name"])
        .unwrap_or_else(|| slug.clone());
    let description = read_text(item, &["summary", "description"]);
    let tags = read_clawhub_tags(item);
    let published_at = read_time(
        item,
        &["createdAt", "created_at", "publishedAt", "published_at"],
    );
    let updated_at = read_time(item, &["updatedAt", "updated_at"]).or_else(|| {
        item.get("latestVersion")
            .and_then(|latest| read_time(latest, &["createdAt", "created_at"]))
    });
    let stats = item.get("stats").unwrap_or(item);
    let download_count = read_number(stats, &["downloads", "downloadCount", "download_count"]);
    let install_count = read_number(
        stats,
        &[
            "installsAllTime",
            "installsCurrent",
            "installCount",
            "install_count",
            "installs",
        ],
    );

    Some(CatalogSkill {
        id: format!("{}::{}", source.id, slug),
        name: title,
        source_id: source.id.clone(),
        source_name: source.name.clone(),
        source_icon: source.icon.clone(),
        source_path: format!("clawhub://{}", slug),
        relative_path: slug,
        description,
        tags,
        supported_agents: read_supported_agents(item),
        published_at,
        updated_at,
        download_count,
        install_count,
        has_skill_md: true,
        has_scripts: false,
        has_references: false,
        has_assets: false,
        install_status: CatalogInstallStatus::NotInstalled,
    })
}

#[cfg(test)]
fn empty_catalog_skill(name: &str) -> CatalogSkill {
    CatalogSkill {
        id: name.to_string(),
        name: name.to_string(),
        source_id: "custom".to_string(),
        source_name: "Custom".to_string(),
        source_icon: "custom".to_string(),
        source_path: String::new(),
        relative_path: name.to_string(),
        description: None,
        tags: Vec::new(),
        supported_agents: Vec::new(),
        published_at: None,
        updated_at: None,
        download_count: None,
        install_count: None,
        has_skill_md: true,
        has_scripts: false,
        has_references: false,
        has_assets: false,
        install_status: CatalogInstallStatus::NotInstalled,
    }
}

fn read_manifest_value(skill_dir: &Path) -> Option<Value> {
    for name in ["skill.json", "skill.yaml", "skill.yml"] {
        let path = skill_dir.join(name);
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(&path).ok()?;
        return match path.extension().and_then(|value| value.to_str()) {
            Some("json") => serde_json::from_str::<Value>(&text).ok(),
            _ => serde_yaml::from_str::<Value>(&text).ok(),
        };
    }
    None
}

fn read_text(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn read_number(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(|candidate| {
            candidate
                .as_u64()
                .or_else(|| {
                    candidate
                        .as_f64()
                        .filter(|value| *value >= 0.0)
                        .map(|value| value as u64)
                })
                .or_else(|| {
                    candidate
                        .as_str()
                        .and_then(|value| value.parse::<u64>().ok())
                })
        })
}

fn read_time(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(|candidate| {
            if let Some(text) = candidate
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Some(text.to_string());
            }
            let millis = candidate
                .as_i64()
                .or_else(|| candidate.as_f64().map(|value| value as i64))?;
            DateTime::<Utc>::from_timestamp_millis(millis).map(|time| time.to_rfc3339())
        })
}

fn read_clawhub_tags(item: &Value) -> Vec<String> {
    let Some(tags) = item.get("tags") else {
        return Vec::new();
    };
    if let Some(values) = tags.as_array() {
        return values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect();
    }
    tags.as_object()
        .map(|values| {
            values
                .keys()
                .filter(|key| key.as_str() != "latest")
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn read_supported_agents(item: &Value) -> Vec<String> {
    let systems = item
        .get("metadata")
        .and_then(|metadata| metadata.get("systems"))
        .or_else(|| item.get("systems"))
        .or_else(|| item.get("supportedAgents"))
        .or_else(|| item.get("supported_agents"));
    let mut agents = systems
        .and_then(|value| {
            if let Some(values) = value.as_array() {
                Some(
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                )
            } else {
                value.as_str().map(|text| {
                    text.split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
            }
        })
        .unwrap_or_default();
    if agents.is_empty() {
        agents.push("openclaw".to_string());
    }
    agents
}

fn infer_supported_agents(source: &CatalogSource) -> Vec<String> {
    match source.id.as_str() {
        "clawhub" => vec!["openclaw".to_string()],
        "claude" => vec!["claude".to_string(), "claudeCode".to_string()],
        "codex" => vec!["codex".to_string()],
        _ => vec!["unknown".to_string()],
    }
}

fn compare_option_desc<T: Ord>(a: Option<T>, b: Option<T>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_option_text_desc(a: &Option<String>, b: &Option<String>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.cmp(a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn source_rank(source_id: &str) -> u8 {
    match source_id {
        "clawhub" => 0,
        "claude" => 1,
        "codex" => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CatalogSort, CatalogSource, CatalogSourceKind};
    use std::fs;

    fn built_in_source(id: &str) -> CatalogSource {
        CatalogSource {
            id: id.to_string(),
            name: id.to_string(),
            url: format!("https://example.com/{id}.git"),
            kind: CatalogSourceKind::BuiltIn,
            icon: id.to_string(),
            enabled: true,
            last_refreshed_at: None,
            cache_path: None,
        }
    }

    #[test]
    fn scans_single_level_skill_md_repository() {
        let root = tempfile::tempdir().unwrap();
        let skill_dir = root.path().join("writer");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: writer\ndescription: Draft clear release notes\n---\n# Writer\n",
        )
        .unwrap();

        let skills = scan_catalog_repository(root.path(), &built_in_source("Claude")).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "writer");
        assert_eq!(
            skills[0].description.as_deref(),
            Some("Draft clear release notes")
        );
        assert!(skills[0].has_skill_md);
    }

    #[test]
    fn scans_two_level_clawhub_repository() {
        let root = tempfile::tempdir().unwrap();
        let skill_dir = root.path().join("alice").join("deploy");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: deploy\ndescription: Deploy projects safely\n---\n# Deploy\n",
        )
        .unwrap();

        let skills = scan_catalog_repository(root.path(), &built_in_source("ClawHub")).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "deploy");
        assert!(
            skills[0].source_path.ends_with("alice\\deploy")
                || skills[0].source_path.ends_with("alice/deploy")
        );
    }

    #[test]
    fn scans_openclaw_clawhub_agents_skill_repository() {
        let root = tempfile::tempdir().unwrap();
        let skill_dir = root
            .path()
            .join(".agents")
            .join("skills")
            .join("alice")
            .join("deploy");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: deploy\ndescription: Deploy projects safely\n---\n# Deploy\n",
        )
        .unwrap();

        let skills = scan_catalog_repository(root.path(), &built_in_source("ClawHub")).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "deploy");
    }

    #[test]
    fn parses_clawhub_api_catalog_items() {
        let source = built_in_source("clawhub");
        let json = r#"{
          "items": [{
            "slug": "self-improving-agent",
            "displayName": "self-improving agent",
            "summary": "Captures learnings and corrections.",
            "tags": {"latest": "3.0.23", "agent": "3.0.0", "memory": "3.0.0"},
            "stats": {"downloads": 459820, "installsAllTime": 6825},
            "createdAt": 1767632598365,
            "updatedAt": 1780785432794,
            "metadata": {"systems": ["OpenClaw", "Claude Code"]}
          }]
        }"#;

        let skills = parse_clawhub_api_catalog(json, &source).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "clawhub::self-improving-agent");
        assert_eq!(skills[0].name, "self-improving agent");
        assert_eq!(skills[0].source_path, "clawhub://self-improving-agent");
        assert_eq!(skills[0].download_count, Some(459820));
        assert_eq!(skills[0].install_count, Some(6825));
        assert!(skills[0].tags.contains(&"agent".to_string()));
        assert!(!skills[0].tags.contains(&"latest".to_string()));
        assert!(skills[0].published_at.is_some());
        assert!(skills[0].updated_at.is_some());
    }

    #[test]
    fn sorts_missing_download_counts_after_available_counts() {
        let mut with_count = empty_catalog_skill("with-count");
        with_count.download_count = Some(10);
        let without_count = empty_catalog_skill("without-count");

        let sorted = sort_catalog_skills(vec![without_count, with_count], CatalogSort::Downloads);

        assert_eq!(sorted[0].name, "with-count");
        assert_eq!(sorted[1].name, "without-count");
    }
}
