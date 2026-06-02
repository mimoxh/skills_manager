use crate::{
    error::{AppError, AppResult},
    models::{AgentProfile, AgentType},
};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub trait AgentAdapter {
    fn detect(&self) -> Vec<AgentProfile>;
    fn validate(&self, profile: &AgentProfile) -> AppResult<()>;
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

}

#[cfg(test)]
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
        // OpenCode 特殊处理：检测 ~/.opencode.json 配置文件
        if self.agent_type == AgentType::OpenCode {
            let config_exists = Self::home_path(&[".opencode.json"])
                .map_or(false, |p| p.exists());
            let skills_path = Self::home_path(&[".opencode", "skills"])
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .map(|h| h.join(".opencode").join("skills").to_string_lossy().to_string())
                        .unwrap_or_default()
                });
            if config_exists || std::path::Path::new(&skills_path).exists() {
                return vec![AgentProfile {
                    id: format!("opencode:{}", skills_path),
                    name: "OpenCode".to_string(),
                    agent_type: AgentType::OpenCode,
                    skills_path,
                    adapter_config: None,
                }];
            }
            return vec![];
        }

        let candidates = match self.agent_type {
            AgentType::Codex => vec![Self::home_path(&[".codex", "skills"])],
            AgentType::Claude => vec![env::var_os("APPDATA")
                .map(PathBuf::from)
                .map(|path| path.join("Claude").join("skills"))],
            AgentType::ClaudeCode => vec![Self::home_path(&[".claude", "skills"])],
            AgentType::Cursor => vec![Self::home_path(&[".cursor", "skills"])],
            AgentType::Trae => vec![Self::home_path(&[".trae", "skills"])],
            AgentType::Custom => vec![],
            AgentType::CherryStudio => vec![env::var_os("APPDATA")
                .map(PathBuf::from)
                .map(|path| path.join("CherryStudio").join("Data").join("Skills"))],
            AgentType::OpenCode => unreachable!(),
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
                        AgentType::Trae => "Trae".to_string(),
                        AgentType::Custom => "Custom".to_string(),
                        AgentType::CherryStudio => "Cherry Studio".to_string(),
                        AgentType::OpenCode => "OpenCode".to_string(),
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

    fn uninstall(
        &self,
        skill_id: &str,
        profile: &AgentProfile,
        _backup_root: &Path,
    ) -> AppResult<Option<PathBuf>> {
        let target = Path::new(&profile.skills_path).join(skill_id);
        if !target.exists() {
            return Ok(None);
        }
        fs::remove_dir_all(&target)?;
        Ok(None)
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
        DirectoryAdapter::new(AgentType::Trae),
        DirectoryAdapter::new(AgentType::CherryStudio),
        DirectoryAdapter::new(AgentType::OpenCode),
    ]
}

/// Check if a profile represents a Cherry Studio agent.
pub fn is_cherry_studio(profile: &AgentProfile) -> bool {
    profile.agent_type == AgentType::CherryStudio
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_path_segment_replaces_windows_reserved_chars() {
        assert_eq!(safe_path_segment("custom:C:\\skills"), "custom_C__skills");
    }
}
