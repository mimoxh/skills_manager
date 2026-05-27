use crate::{
    error::{AppError, AppResult},
    hash::{copy_dir_all, hash_dir},
    models::{
        AgentProfile, AgentType, ConflictPolicy, InstallResult, InstallState, InstallStatus,
        SkillSummary,
    },
};
use chrono::Utc;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub trait AgentAdapter {
    fn detect(&self) -> Vec<AgentProfile>;
    fn validate(&self, profile: &AgentProfile) -> AppResult<()>;
    fn check_compatibility(&self, skill: &SkillSummary, profile: &AgentProfile) -> bool;
    fn diff(
        &self,
        skill: &SkillSummary,
        profile: &AgentProfile,
        installed: Option<String>,
    ) -> AppResult<InstallState>;
    fn install(
        &self,
        skill: &SkillSummary,
        profile: &AgentProfile,
        policy: ConflictPolicy,
        backup_root: &Path,
    ) -> AppResult<InstallResult>;
    fn uninstall(
        &self,
        skill_id: &str,
        profile: &AgentProfile,
        backup_root: &Path,
    ) -> AppResult<Option<PathBuf>>;
}

#[derive(Clone)]
pub struct DirectoryAdapter {
    agent_type: AgentType,
}

impl DirectoryAdapter {
    pub fn new(agent_type: AgentType) -> Self {
        Self { agent_type }
    }

    fn home_path(parts: &[&str]) -> Option<PathBuf> {
        let mut path = dirs::home_dir()?;
        for part in parts {
            path.push(part);
        }
        Some(path)
    }

    fn target_path(&self, skill: &SkillSummary, profile: &AgentProfile) -> PathBuf {
        Path::new(&profile.skills_path).join(&skill.manifest.id)
    }

    fn backup_path(&self, backup_root: &Path, profile: &AgentProfile, skill_id: &str) -> PathBuf {
        backup_root
            .join(safe_path_segment(&profile.id))
            .join(safe_path_segment(skill_id))
            .join(Utc::now().format("%Y%m%d%H%M%S").to_string())
    }
}

fn safe_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ if ch.is_control() => '_',
            _ => ch,
        })
        .collect()
}

impl AgentAdapter for DirectoryAdapter {
    fn detect(&self) -> Vec<AgentProfile> {
        let candidates = match self.agent_type {
            AgentType::Codex => vec![Self::home_path(&[".codex", "skills"])],
            AgentType::Claude => vec![env::var_os("APPDATA")
                .map(PathBuf::from)
                .map(|path| path.join("Claude").join("skills"))],
            AgentType::ClaudeCode => vec![Self::home_path(&[".claude", "skills"])],
            AgentType::Cursor => vec![Self::home_path(&[".cursor", "skills"])],
            AgentType::Windsurf => vec![Self::home_path(&[".windsurf", "skills"])],
            AgentType::Aider => vec![Self::home_path(&[".aider", "skills"])],
            AgentType::Custom => vec![],
        };

        candidates
            .into_iter()
            .flatten()
            .filter(|path| path.exists())
            .map(|path| {
                let type_name = self.agent_type.as_str();
                AgentProfile {
                    id: format!("{}:{}", type_name, path.to_string_lossy()),
                    name: match self.agent_type {
                        AgentType::Codex => "Codex".to_string(),
                        AgentType::Claude => "Claude".to_string(),
                        AgentType::ClaudeCode => "Claude Code".to_string(),
                        AgentType::Cursor => "Cursor".to_string(),
                        AgentType::Windsurf => "Windsurf".to_string(),
                        AgentType::Aider => "Aider".to_string(),
                        AgentType::Custom => "Custom".to_string(),
                    },
                    agent_type: self.agent_type.clone(),
                    skills_path: path.to_string_lossy().to_string(),
                    adapter_config: None,
                }
            })
            .collect()
    }

    fn validate(&self, profile: &AgentProfile) -> AppResult<()> {
        if profile.skills_path.trim().is_empty() {
            return Err(AppError::Message(
                "Agent skillsPath is required".to_string(),
            ));
        }
        fs::create_dir_all(&profile.skills_path)?;
        Ok(())
    }

    fn check_compatibility(&self, skill: &SkillSummary, profile: &AgentProfile) -> bool {
        let agent_key = profile.agent_type.as_str();
        skill
            .manifest
            .supported_agents
            .iter()
            .any(|item| item == "*" || item.eq_ignore_ascii_case(agent_key))
    }

    fn diff(
        &self,
        skill: &SkillSummary,
        profile: &AgentProfile,
        installed: Option<String>,
    ) -> AppResult<InstallState> {
        let target = self.target_path(skill, profile);
        if !self.check_compatibility(skill, profile) {
            return Ok(InstallState {
                agent_id: profile.id.clone(),
                skill_id: skill.manifest.id.clone(),
                status: InstallStatus::Missing,
                installed_fingerprint: installed,
                target_fingerprint: None,
                message: "Not compatible with this agent".to_string(),
            });
        }
        if !target.exists() {
            return Ok(InstallState {
                agent_id: profile.id.clone(),
                skill_id: skill.manifest.id.clone(),
                status: InstallStatus::Missing,
                installed_fingerprint: installed,
                target_fingerprint: None,
                message: "Not installed".to_string(),
            });
        }
        let target_fingerprint = hash_dir(&target)?;
        let status = if target_fingerprint == skill.fingerprint {
            InstallStatus::Installed
        } else if installed.as_deref() == Some(target_fingerprint.as_str()) {
            InstallStatus::Stale
        } else {
            InstallStatus::Conflict
        };
        let message = match status {
            InstallStatus::Installed => "Installed and current",
            InstallStatus::Stale => "Installed but source has changed",
            InstallStatus::Conflict => "Target was modified outside this app",
            InstallStatus::Missing => "Not installed",
        }
        .to_string();
        Ok(InstallState {
            agent_id: profile.id.clone(),
            skill_id: skill.manifest.id.clone(),
            status,
            installed_fingerprint: installed,
            target_fingerprint: Some(target_fingerprint),
            message,
        })
    }

    fn install(
        &self,
        skill: &SkillSummary,
        profile: &AgentProfile,
        policy: ConflictPolicy,
        backup_root: &Path,
    ) -> AppResult<InstallResult> {
        self.validate(profile)?;
        if !self.check_compatibility(skill, profile) {
            return Err(AppError::Message(format!(
                "{} is not compatible with {}",
                skill.manifest.name, profile.name
            )));
        }

        let source = Path::new(&skill.source_path);
        let mut target = self.target_path(skill, profile);
        let mut action = if target.exists() {
            "updated"
        } else {
            "installed"
        }
        .to_string();
        let mut backup_path = None;

        if target.exists() {
            match policy {
                ConflictPolicy::Prompt => {
                    return Err(AppError::Message(
                        "Conflict policy is prompt; choose an explicit install policy first"
                            .to_string(),
                    ));
                }
                ConflictPolicy::Skip => {
                    return Ok(InstallResult {
                        agent_id: profile.id.clone(),
                        skill_id: skill.manifest.id.clone(),
                        action: "skipped".to_string(),
                        target_path: target.to_string_lossy().to_string(),
                        backup_path: None,
                        message: format!("Skipped {}", skill.manifest.name),
                    });
                }
                ConflictPolicy::Rename => {
                    let suffix = Utc::now().format("%Y%m%d%H%M%S").to_string();
                    target = Path::new(&profile.skills_path)
                        .join(format!("{}-{}", skill.manifest.id, suffix));
                    action = "renamed".to_string();
                }
                ConflictPolicy::BackupOverwrite => {
                    let backup = self.backup_path(backup_root, profile, &skill.manifest.id);
                    copy_dir_all(&target, &backup)?;
                    fs::remove_dir_all(&target)?;
                    backup_path = Some(backup.to_string_lossy().to_string());
                }
            }
        }
        copy_dir_all(source, &target)?;
        Ok(InstallResult {
            agent_id: profile.id.clone(),
            skill_id: skill.manifest.id.clone(),
            action,
            target_path: target.to_string_lossy().to_string(),
            backup_path,
            message: format!("{} synced to {}", skill.manifest.name, profile.name),
        })
    }

    fn uninstall(
        &self,
        skill_id: &str,
        profile: &AgentProfile,
        backup_root: &Path,
    ) -> AppResult<Option<PathBuf>> {
        let target = Path::new(&profile.skills_path).join(skill_id);
        if !target.exists() {
            return Ok(None);
        }
        let backup = self.backup_path(backup_root, profile, skill_id);
        copy_dir_all(&target, &backup)?;
        fs::remove_dir_all(&target)?;
        Ok(Some(backup))
    }
}

pub fn adapter_for(profile: &AgentProfile) -> DirectoryAdapter {
    DirectoryAdapter::new(profile.agent_type.clone())
}

pub fn built_in_adapters() -> Vec<DirectoryAdapter> {
    vec![
        DirectoryAdapter::new(AgentType::Codex),
        DirectoryAdapter::new(AgentType::Claude),
        DirectoryAdapter::new(AgentType::ClaudeCode),
        DirectoryAdapter::new(AgentType::Cursor),
        DirectoryAdapter::new(AgentType::Windsurf),
        DirectoryAdapter::new(AgentType::Aider),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_accepts_wildcard() {
        let skill = SkillSummary {
            manifest: crate::models::SkillManifest {
                id: "demo".into(),
                name: "Demo".into(),
                version: "1".into(),
                description: None,
                tags: vec![],
                supported_agents: vec!["*".into()],
                entry: None,
                files: vec!["SKILL.md".into()],
            },
            source_path: ".".into(),
            fingerprint: "abc".into(),
            manifest_path: "./skill.json".into(),
        };
        let profile = AgentProfile {
            id: "custom:test".into(),
            name: "Custom".into(),
            agent_type: AgentType::Custom,
            skills_path: ".".into(),
            adapter_config: None,
        };
        assert!(DirectoryAdapter::new(AgentType::Custom).check_compatibility(&skill, &profile));
    }

    #[test]
    fn installs_and_detects_conflict_before_overwrite() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("skill.json"), "{}").unwrap();
        std::fs::write(source.path().join("SKILL.md"), "hello").unwrap();

        let skill = SkillSummary {
            manifest: crate::models::SkillManifest {
                id: "demo".into(),
                name: "Demo".into(),
                version: "1".into(),
                description: None,
                tags: vec![],
                supported_agents: vec!["custom".into()],
                entry: None,
                files: vec!["SKILL.md".into()],
            },
            source_path: source.path().to_string_lossy().to_string(),
            fingerprint: crate::hash::hash_dir(source.path()).unwrap(),
            manifest_path: source
                .path()
                .join("skill.json")
                .to_string_lossy()
                .to_string(),
        };
        let profile = AgentProfile {
            id: "custom:test".into(),
            name: "Custom".into(),
            agent_type: AgentType::Custom,
            skills_path: target.path().to_string_lossy().to_string(),
            adapter_config: None,
        };
        let adapter = DirectoryAdapter::new(AgentType::Custom);

        let first = adapter
            .install(
                &skill,
                &profile,
                ConflictPolicy::BackupOverwrite,
                backup.path(),
            )
            .unwrap();
        assert_eq!(first.action, "installed");
        std::fs::write(target.path().join("demo").join("SKILL.md"), "changed").unwrap();

        let state = adapter
            .diff(&skill, &profile, Some(skill.fingerprint.clone()))
            .unwrap();
        assert_eq!(state.status, InstallStatus::Conflict);

        let second = adapter
            .install(
                &skill,
                &profile,
                ConflictPolicy::BackupOverwrite,
                backup.path(),
            )
            .unwrap();
        assert_eq!(second.action, "updated");
        assert!(second.backup_path.is_some());
        assert_eq!(
            std::fs::read_to_string(target.path().join("demo").join("SKILL.md")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn safe_path_segment_replaces_windows_reserved_chars() {
        assert_eq!(safe_path_segment("custom:C:\\skills"), "custom_C__skills");
    }
}
