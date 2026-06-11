use crate::{
    error::{AppError, AppResult},
    models::{CatalogInstallStatus, CatalogSkill, CatalogSort, CatalogSource},
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{cmp::Ordering, fs, path::Path};
use walkdir::WalkDir;

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
        .max_depth(4)
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

fn read_skill_md_frontmatter(text: &str) -> Option<(Option<String>, Option<String>, Vec<String>)> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_first = &trimmed[3..];
    let end_idx = after_first.find("\n---")?;
    let frontmatter = &after_first[..end_idx];
    let value = serde_yaml::from_str::<Value>(frontmatter).ok()?;
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let description = value
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let tags = value
        .get("tags")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some((name, description, tags))
}

fn read_markdown_heading(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
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
    fn sorts_missing_download_counts_after_available_counts() {
        let mut with_count = empty_catalog_skill("with-count");
        with_count.download_count = Some(10);
        let without_count = empty_catalog_skill("without-count");

        let sorted = sort_catalog_skills(vec![without_count, with_count], CatalogSort::Downloads);

        assert_eq!(sorted[0].name, "with-count");
        assert_eq!(sorted[1].name, "without-count");
    }
}
