use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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
    #[serde(default)]
    pub readme: Option<String>,
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
    #[serde(default)]
    pub readme: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    #[serde(rename = "trae")]
    Trae,
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "cherryStudio")]
    CherryStudio,
    #[serde(rename = "opencode")]
    OpenCode,
}

impl AgentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::Codex => "codex",
            AgentType::Claude => "claude",
            AgentType::ClaudeCode => "claudeCode",
            AgentType::Cursor => "cursor",
            AgentType::Trae => "trae",
            AgentType::Custom => "custom",
            AgentType::CherryStudio => "cherryStudio",
            AgentType::OpenCode => "opencode",
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
    #[serde(default)]
    pub no_full_coverage_titles: Vec<String>,
    #[serde(default)]
    pub no_full_coverage_mcp_titles: Vec<String>,
}

// ── MCP 数据模型 ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum McpTransport {
    #[serde(rename = "stdio")]
    Stdio,
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "sse")]
    Sse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub name: String,
    pub transport: McpTransport,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub timeout_sec: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMcpServer {
    pub agent_id: String,
    pub agent_name: String,
    pub config_path: String,
    pub config: McpServerConfig,
    pub fingerprint: String,
    #[serde(default)]
    pub raw_config: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GroupedMcpServer {
    pub name: String,
    pub copies: Vec<AgentMcpServer>,
    pub agent_ids: Vec<String>,
    #[serde(default)]
    pub disabled_agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOperationResult {
    pub agent_id: String,
    pub server_name: String,
    pub action: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTestResult {
    pub agent_id: String,
    pub server_name: String,
    pub success: bool,
    pub message: String,
}
