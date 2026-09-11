use crate::{
    error::{AppError, AppResult},
    models::{AgentProfile, AgentType},
    util::effective_supports_universal,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub trait AgentAdapter {
    fn detect(&self) -> Vec<AgentProfile>;
    fn validate(&self, profile: &AgentProfile) -> AppResult<()>;
    fn uninstall(&self, skill_id: &str, profile: &AgentProfile) -> AppResult<Option<PathBuf>>;
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
        home_dir_path(parts)
    }

    fn detect_claude_cowork_profiles(session_root: &Path) -> Vec<AgentProfile> {
        let skills_plugin_root = session_root.join("skills-plugin");
        let Ok(workspaces) = fs::read_dir(&skills_plugin_root) else {
            return Vec::new();
        };
        let mut profiles = Vec::new();
        for workspace in workspaces.filter_map(Result::ok) {
            let Ok(file_type) = workspace.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let Ok(plugins) = fs::read_dir(workspace.path()) else {
                continue;
            };
            for plugin in plugins.filter_map(Result::ok) {
                let Ok(file_type) = plugin.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }
                let plugin_root = plugin.path();
                let manifest_path = plugin_root.join("manifest.json");
                let skills_path = plugin_root.join("skills");
                let plugin_json = plugin_root.join(".claude-plugin").join("plugin.json");
                if !(manifest_path.exists() && skills_path.is_dir() && plugin_json.exists()) {
                    continue;
                }
                profiles.push(AgentProfile {
                    id: format!("claudeCowork:{}", plugin_root.to_string_lossy()),
                    name: "Claude Desktop Cowork".to_string(),
                    agent_type: AgentType::ClaudeCowork,
                    skills_path: skills_path.to_string_lossy().to_string(),
                    adapter_config: Some(serde_json::json!({
                        "pluginRoot": plugin_root.to_string_lossy(),
                        "manifestPath": manifest_path.to_string_lossy()
                    })),
                    user_tags: Vec::new(),
                    supports_universal: false,
                });
            }
        }
        profiles.sort_by(|a, b| a.skills_path.cmp(&b.skills_path));
        profiles
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
        // OpenCode 特殊处理：检测 ~/.config/opencode 配置目录
        if self.agent_type == AgentType::OpenCode {
            let Some(config_dir) = opencode_config_dir() else {
                return vec![];
            };
            let config_exists = config_dir.join("opencode.json").exists()
                || config_dir.join("opencode.jsonc").exists()
                || Self::home_path(&[".opencode.json"]).map_or(false, |p| p.exists());
            let skills_dir = config_dir.join("skills");
            if config_exists || config_dir.is_dir() || skills_dir.exists() {
                let skills_path = skills_dir.to_string_lossy().to_string();
                return vec![AgentProfile {
                    id: format!("opencode:{}", skills_path),
                    name: "OpenCode".to_string(),
                    agent_type: AgentType::OpenCode,
                    skills_path,
                    adapter_config: None,
                    user_tags: Vec::new(),
                    // OpenCode 原生扫描全局 `~/.agents/skills`，无需重复安装
                    supports_universal: true,
                }];
            }
            return vec![];
        }

        if self.agent_type == AgentType::ClaudeCowork {
            return env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .map(|path| {
                    Self::detect_claude_cowork_profiles(
                        &path.join("Claude-3p").join("local-agent-mode-sessions"),
                    )
                })
                .unwrap_or_default();
        }

        let candidates = match self.agent_type {
            AgentType::Universal => vec![Self::home_path(&[".agents", "skills"])],
            AgentType::Codex => vec![Self::home_path(&[".codex", "skills"])],
            AgentType::Claude => vec![
                env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .map(|path| path.join("Claude").join("skills")),
            ],
            AgentType::ClaudeCode => vec![Self::home_path(&[".claude", "skills"])],
            AgentType::ClaudeCowork => unreachable!(),
            AgentType::Cursor => vec![Self::home_path(&[".cursor", "skills"])],
            AgentType::Trae => vec![Self::home_path(&[".trae", "skills"])],
            AgentType::Custom => vec![],
            AgentType::CherryStudio => vec![
                env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .map(|path| path.join("CherryStudio").join("Data").join("Skills")),
            ],
            AgentType::OpenCode => unreachable!(),
        };

        candidates
            .into_iter()
            .flatten()
            .filter(|path| path.exists())
            .map(|path| {
                let type_name = self.agent_type.as_str();
                let path_str = path.to_string_lossy();
                AgentProfile {
                    id: format!("{}:{}", type_name, path_str),
                    name: match self.agent_type {
                        AgentType::Universal => "Universal (.agents/skills)".to_string(),
                        AgentType::Codex => "Codex".to_string(),
                        AgentType::Claude => "Claude".to_string(),
                        AgentType::ClaudeCode => "Claude Code".to_string(),
                        AgentType::ClaudeCowork => "Claude Desktop Cowork".to_string(),
                        AgentType::Cursor => "Cursor".to_string(),
                        AgentType::Trae => "Trae".to_string(),
                        AgentType::Custom => "Custom".to_string(),
                        AgentType::CherryStudio => "Cherry Studio".to_string(),
                        AgentType::OpenCode => "OpenCode".to_string(),
                    },
                    agent_type: self.agent_type.clone(),
                    skills_path: path_str.to_string(),
                    adapter_config: None,
                    user_tags: Vec::new(),
                    supports_universal: effective_supports_universal(
                        self.agent_type == AgentType::Universal,
                        &path_str,
                        false,
                    ),
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
    ) -> AppResult<Option<PathBuf>> {
        let target = Path::new(&profile.skills_path).join(skill_id);
        if !target.exists() && fs::symlink_metadata(&target).is_err() {
            return Ok(None);
        }
        crate::hash::remove_dir_or_symlink(&target)?;
        Ok(None)
    }
}

pub fn adapter_for(profile: &AgentProfile) -> DirectoryAdapter {
    DirectoryAdapter::new(profile.agent_type.clone())
}

/// 拼接用户主目录下的路径。
fn home_dir_path(parts: &[&str]) -> Option<PathBuf> {
    let mut path = dirs::home_dir()?;
    for part in parts {
        path.push(part);
    }
    Some(path)
}

/// OpenCode 全局配置目录。优先 `XDG_CONFIG_HOME/opencode`，
/// 否则回退到 `~/.config/opencode`（Windows 上同样使用该路径）。
fn opencode_config_dir() -> Option<PathBuf> {
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("opencode"));
        }
    }
    dirs::home_dir().map(|home| home.join(".config").join("opencode"))
}

/// 内置 Agent 类型的默认 Skills 目录（用于新增未检测到的 Agent）。
/// `Custom` 与 `ClaudeCowork` 没有可推断的默认目录，返回 `None`。
pub fn default_skills_path(agent_type: &AgentType) -> Option<PathBuf> {
    match agent_type {
        AgentType::Universal => home_dir_path(&[".agents", "skills"]),
        AgentType::Codex => home_dir_path(&[".codex", "skills"]),
        AgentType::ClaudeCode => home_dir_path(&[".claude", "skills"]),
        AgentType::Cursor => home_dir_path(&[".cursor", "skills"]),
        AgentType::Trae => home_dir_path(&[".trae", "skills"]),
        AgentType::OpenCode => opencode_config_dir().map(|dir| dir.join("skills")),
        AgentType::Claude => env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Claude").join("skills")),
        AgentType::CherryStudio => env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("CherryStudio").join("Data").join("Skills")),
        AgentType::ClaudeCowork | AgentType::Custom => None,
    }
}

pub fn built_in_adapters() -> Vec<DirectoryAdapter> {
    vec![
        DirectoryAdapter::new(AgentType::Universal),
        DirectoryAdapter::new(AgentType::Codex),
        DirectoryAdapter::new(AgentType::Claude),
        DirectoryAdapter::new(AgentType::ClaudeCode),
        DirectoryAdapter::new(AgentType::ClaudeCowork),
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

    #[test]
    fn detects_claude_cowork_plugin_package() {
        let root = tempfile::tempdir().unwrap();
        let plugin_root = root
            .path()
            .join("skills-plugin")
            .join("00000000-0000-4000-8000-000000000001")
            .join("44b558d3-efb7-4a3f-94f2-b659b67adebd");
        fs::create_dir_all(plugin_root.join(".claude-plugin")).unwrap();
        fs::create_dir_all(plugin_root.join("skills")).unwrap();
        fs::write(plugin_root.join(".claude-plugin").join("plugin.json"), "{}").unwrap();
        fs::write(plugin_root.join("manifest.json"), r#"{"skills":[]}"#).unwrap();

        let profiles = DirectoryAdapter::detect_claude_cowork_profiles(root.path());

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].agent_type, AgentType::ClaudeCowork);
        assert_eq!(profiles[0].name, "Claude Desktop Cowork");
        assert_eq!(
            profiles[0].skills_path,
            plugin_root.join("skills").to_string_lossy()
        );
        let config = profiles[0].adapter_config.as_ref().unwrap();
        assert_eq!(
            config.get("pluginRoot").and_then(|value| value.as_str()),
            Some(plugin_root.to_string_lossy().as_ref())
        );
        assert_eq!(
            config.get("manifestPath").and_then(|value| value.as_str()),
            Some(plugin_root.join("manifest.json").to_string_lossy().as_ref())
        );
    }

    #[test]
    fn default_skills_path_covers_builtin_types() {
        let opencode = default_skills_path(&AgentType::OpenCode).unwrap();
        assert!(opencode.ends_with(Path::new("opencode").join("skills")));
        assert!(default_skills_path(&AgentType::Codex).is_some());
        assert!(default_skills_path(&AgentType::Custom).is_none());
        assert!(default_skills_path(&AgentType::ClaudeCowork).is_none());
    }
}
