use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub supported_agents: Vec<String>,
    #[serde(default)]
    pub entry: Option<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub manifest: SkillManifest,
    pub source_path: String,
    pub fingerprint: String,
    pub manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkillCopy {
    pub agent_id: String,
    pub agent_name: String,
    pub skill_path: String,
    pub title: String,
    pub version: Option<String>,
    pub fingerprint: String,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GroupedSkill {
    pub title: String,
    pub best_copy: AgentSkillCopy,
    pub copies: Vec<AgentSkillCopy>,
    pub installed_agent_ids: Vec<String>,
    pub missing_agent_ids: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkillFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkillResult {
    pub imported: usize,
    pub skipped: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentType {
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "claude")]
    Claude,
    #[serde(rename = "claudeCode")]
    ClaudeCode,
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "windsurf")]
    Windsurf,
    #[serde(rename = "aider")]
    Aider,
    #[serde(rename = "custom")]
    Custom,
}

impl AgentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::Codex => "codex",
            AgentType::Claude => "claude",
            AgentType::ClaudeCode => "claudeCode",
            AgentType::Cursor => "cursor",
            AgentType::Windsurf => "windsurf",
            AgentType::Aider => "aider",
            AgentType::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryPathEntry {
    pub path: String,
    pub label: String,
    pub skills_subdir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub agent_type: AgentType,
    pub skills_path: String,
    #[serde(default)]
    pub adapter_config: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    #[serde(rename = "prompt")]
    Prompt,
    #[serde(rename = "backupOverwrite")]
    BackupOverwrite,
    #[serde(rename = "skip")]
    Skip,
    #[serde(rename = "rename")]
    Rename,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub agent_id: String,
    pub skill_id: String,
    pub action: String,
    pub target_path: String,
    pub backup_path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialData {
    pub skills: Vec<GroupedSkill>,
    pub agents: Vec<AgentProfile>,
}
