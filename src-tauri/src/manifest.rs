use crate::{
    error::{AppError, AppResult},
    hash::hash_dir,
    models::{SkillManifest, SkillSummary},
};
use std::{
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
