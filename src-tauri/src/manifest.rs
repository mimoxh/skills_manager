use crate::{
    error::{AppError, AppResult},
    hash::hash_dir,
    models::{SkillManifest, SkillSummary},
};
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

pub fn scan_repository(repository: &Path) -> AppResult<Vec<SkillSummary>> {
    if !repository.exists() {
        return Err(AppError::Message(format!(
            "Skills repository does not exist: {}",
            repository.display()
        )));
    }
    let mut manifests = Vec::new();
    for entry in WalkDir::new(repository)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name == "skill.json" || name == "skill.yaml" || name == "skill.yml" {
            manifests.push(read_skill(entry.path())?);
        }
    }
    manifests.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(manifests)
}

pub fn read_skill(manifest_path: &Path) -> AppResult<SkillSummary> {
    let text = fs::read_to_string(manifest_path)?;
    let manifest = match manifest_path.extension().and_then(|value| value.to_str()) {
        Some("json") => serde_json::from_str::<SkillManifest>(&text)?,
        _ => serde_yaml::from_str::<SkillManifest>(&text)?,
    };
    validate_manifest(&manifest)?;
    let source = manifest_path.parent().map(PathBuf::from).ok_or_else(|| {
        AppError::Message("Skill manifest must have a parent directory".to_string())
    })?;
    Ok(SkillSummary {
        manifest,
        fingerprint: hash_dir(&source)?,
        source_path: source.to_string_lossy().to_string(),
        manifest_path: manifest_path.to_string_lossy().to_string(),
    })
}

pub fn validate_manifest(manifest: &SkillManifest) -> AppResult<()> {
    if manifest.id.trim().is_empty() {
        return Err(AppError::Message("Skill id is required".to_string()));
    }
    if manifest.name.trim().is_empty() {
        return Err(AppError::Message(format!(
            "Skill {} is missing name",
            manifest.id
        )));
    }
    if manifest.version.trim().is_empty() {
        return Err(AppError::Message(format!(
            "Skill {} is missing version",
            manifest.id
        )));
    }
    if manifest.supported_agents.is_empty() {
        return Err(AppError::Message(format!(
            "Skill {} must declare supportedAgents",
            manifest.id
        )));
    }
    if manifest.files.is_empty() {
        return Err(AppError::Message(format!(
            "Skill {} must declare files",
            manifest.id
        )));
    }
    Ok(())
}

/// Read YAML frontmatter from a SKILL.md file.
/// Returns (name, description, tags) extracted from the frontmatter.
pub fn read_skill_md_frontmatter(text: &str) -> Option<(Option<String>, Option<String>, Vec<String>)> {
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
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);
    let description = value
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
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

/// Extract the first markdown heading (# ...) from text.
pub fn read_markdown_heading(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

/// Extract version from SKILL.md frontmatter, supporting both top-level `version`
/// and nested `metadata.version`.
fn extract_version_from_frontmatter(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_first = &trimmed[3..];
    let end_idx = after_first.find("\n---")?;
    let frontmatter = &after_first[..end_idx];
    let value = serde_yaml::from_str::<Value>(frontmatter).ok()?;
    // Try top-level version first
    if let Some(v) = value.get("version").and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
        return Some(v.to_string());
    }
    // Fall back to metadata.version
    value
        .get("metadata")
        .and_then(|m| m.get("version"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

/// Scan a directory for SKILL.md files (no manifest companion) and synthesize
/// `SkillSummary` entries from their frontmatter. Used as fallback during import
/// when no `skill.json`/`skill.yaml`/`skill.yml` is found.
pub fn scan_skill_md_only(repository: &Path) -> AppResult<Vec<SkillSummary>> {
    if !repository.exists() {
        return Ok(Vec::new());
    }
    let mut results = Vec::new();
    let mut seen = HashSet::new();
    for entry in WalkDir::new(repository)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name != "skill.md" {
            continue;
        }
        let skill_md_path = entry.path();
        let source = match skill_md_path.parent() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        // Skip if this directory already has a manifest file (covered by scan_repository)
        if has_manifest(&source) {
            continue;
        }
        if !seen.insert(source.clone()) {
            continue;
        }
        match synthesize_manifest_from_skill_md(skill_md_path) {
            Ok(summary) => results.push(summary),
            Err(_) => continue, // Skip unparseable SKILL.md files
        }
    }
    results.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(results)
}

/// Check if a directory contains a manifest file.
fn has_manifest(dir: &Path) -> bool {
    ["skill.json", "skill.yaml", "skill.yml"]
        .iter()
        .any(|name| dir.join(name).exists())
}

/// Synthesize a `SkillSummary` from a SKILL.md file's frontmatter.
pub fn synthesize_manifest_from_skill_md(skill_md_path: &Path) -> AppResult<SkillSummary> {
    let text = fs::read_to_string(skill_md_path)?;
    let source = skill_md_path.parent().ok_or_else(|| {
        AppError::Message("SKILL.md must have a parent directory".to_string())
    })?;
    let dir_name = source
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("unknown")
        .to_string();

    let (fm_name, fm_description, _tags) = read_skill_md_frontmatter(&text)
        .unwrap_or((None, None, Vec::new()));
    let heading = read_markdown_heading(&text);
    let version = extract_version_from_frontmatter(&text)
        .unwrap_or_else(|| "1.0.0".to_string());

    let name = fm_name
        .or(heading)
        .unwrap_or_else(|| dir_name.clone());

    // Collect all non-manifest files in the directory as skill files
    let files = collect_skill_files(source);

    let manifest = SkillManifest {
        id: dir_name,
        name,
        version,
        description: fm_description,
        tags: Vec::new(),
        supported_agents: vec!["*".to_string()],
        entry: None,
        files,
    };
    validate_manifest(&manifest)?;
    Ok(SkillSummary {
        manifest,
        fingerprint: hash_dir(source)?,
        source_path: source.to_string_lossy().to_string(),
        manifest_path: skill_md_path.to_string_lossy().to_string(),
    })
}

/// Collect all files in a skill directory (non-recursive, excluding manifest files).
fn collect_skill_files(dir: &Path) -> Vec<String> {
    let manifest_names: HashSet<&str> = ["skill.json", "skill.yaml", "skill.yml", "skill.md"]
        .iter().copied().collect();
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if !manifest_names.contains(name.as_str()) {
                    files.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
    }
    // Always include SKILL.md as a skill file
    if dir.join("SKILL.md").exists() {
        files.push("SKILL.md".to_string());
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_id() {
        let manifest = SkillManifest {
            id: "".into(),
            name: "Demo".into(),
            version: "1.0.0".into(),
            description: None,
            tags: vec![],
            supported_agents: vec!["codex".into()],
            entry: None,
            files: vec!["SKILL.md".into()],
        };
        assert!(validate_manifest(&manifest).is_err());
    }
}
