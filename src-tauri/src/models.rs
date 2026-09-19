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
    pub updated_at: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default = "default_true")]
    pub is_registered: bool,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub installed_at: Option<String>,
    #[serde(default)]
    pub is_symlink: bool,
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
    pub user_tags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub installed_at: Option<String>,
    #[serde(default)]
    pub is_universal: bool,
    #[serde(default)]
    pub universal_agent_id: Option<String>,
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
    Universal,
    Codex,
    Claude,
    ClaudeCode,
    ClaudeCowork,
    Cursor,
    Trae,
    Custom,
    CherryStudio,
    OpenCode,
}

impl AgentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::Universal => "universal",
            AgentType::Codex => "codex",
            AgentType::Claude => "claude",
            AgentType::ClaudeCode => "claudeCode",
            AgentType::ClaudeCowork => "claudeCowork",
            AgentType::Cursor => "cursor",
            AgentType::Trae => "trae",
            AgentType::Custom => "custom",
            AgentType::CherryStudio => "cherryStudio",
            AgentType::OpenCode => "opencode",
        }
    }

    /// 内置 Agent 类型的默认展示名；自定义类型没有默认名称。
    pub fn default_name(&self) -> Option<&'static str> {
        match self {
            AgentType::Universal => Some("Universal (.agents/skills)"),
            AgentType::Codex => Some("Codex"),
            AgentType::Claude => Some("Claude"),
            AgentType::ClaudeCode => Some("Claude Code"),
            AgentType::ClaudeCowork => Some("Claude Desktop Cowork"),
            AgentType::Cursor => Some("Cursor"),
            AgentType::Trae => Some("Trae"),
            AgentType::CherryStudio => Some("Cherry Studio"),
            AgentType::OpenCode => Some("OpenCode"),
            AgentType::Custom => None,
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
    #[serde(default)]
    pub user_tags: Vec<String>,
    #[serde(default)]
    pub supports_universal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    Prompt,
    BackupOverwrite,
    Skip,
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
    /// 内置默认 catalog 源 id（当前为 "clawhub"），由后端下发，避免前端硬编码。
    #[serde(default)]
    pub default_catalog_source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CatalogSourceKind {
    BuiltIn,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSource {
    pub id: String,
    pub name: String,
    pub url: String,
    pub kind: CatalogSourceKind,
    pub icon: String,
    pub enabled: bool,
    #[serde(default)]
    pub last_refreshed_at: Option<String>,
    #[serde(default)]
    pub cache_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CatalogSort {
    Downloads,
    PublishedDesc,
    UpdatedDesc,
    Source,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CatalogInstallStatus {
    NotInstalled,
    Installed,
    UpdateAvailable,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSkill {
    pub id: String,
    pub name: String,
    pub source_id: String,
    pub source_name: String,
    pub source_icon: String,
    pub source_path: String,
    pub relative_path: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub supported_agents: Vec<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub download_count: Option<u64>,
    #[serde(default)]
    pub install_count: Option<u64>,
    pub has_skill_md: bool,
    pub has_scripts: bool,
    pub has_references: bool,
    pub has_assets: bool,
    pub install_status: CatalogInstallStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum CatalogSafetyMode {
    #[default]
    All,
    NonSuspicious,
}

impl CatalogSafetyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            CatalogSafetyMode::All => "all",
            CatalogSafetyMode::NonSuspicious => "nonSuspicious",
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSearchResult {
    pub items: Vec<CatalogSkill>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFilters {
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub agent_types: Vec<String>,
    #[serde(default)]
    pub install_statuses: Vec<CatalogInstallStatus>,
    #[serde(default)]
    pub has_download_data: Option<bool>,
    #[serde(default)]
    pub time_window_days: Option<i64>,
    #[serde(default)]
    pub content_capabilities: Vec<String>,
    #[serde(default)]
    pub safety_mode: CatalogSafetyMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRefreshResult {
    pub source_id: String,
    pub refreshed: bool,
    pub skill_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRefreshStatus {
    pub source_id: String,
    pub safety_mode: CatalogSafetyMode,
    pub is_running: bool,
    pub is_complete: bool,
    pub fetched_count: usize,
    pub next_cursor: Option<String>,
    pub generation: i64,
    pub last_error: Option<String>,
    pub updated_at: Option<String>,
}

// ── MCP 数据模型 ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum McpTransport {
    Stdio,
    Http,
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

// ── Skills 同步数据模型 ───────────────────────────────────────────────

/// 同步配置。`secretAccessKey` 与加密口令不在此结构内（存 OS 钥匙串）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default = "default_true")]
    pub path_style: bool,
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default = "default_poll_secs")]
    pub poll_secs: u64,
    #[serde(default = "default_true")]
    pub encrypt: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            bucket: String::new(),
            region: default_region(),
            path_style: true,
            access_key_id: String::new(),
            poll_secs: default_poll_secs(),
            encrypt: true,
        }
    }
}

/// 同步状态（下发给前端徽标 / 设置页）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub configured: bool,
    pub enabled: bool,
    pub running: bool,
    #[serde(default)]
    pub last_run_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    pub pending_conflicts: usize,
    pub device_id: String,
    pub device_name: String,
}

/// 单台设备发布的清单（加密前的明文结构）。存于 `devices/<deviceId>.json`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceManifest {
    pub schema_version: u32,
    pub device_id: String,
    pub device_name: String,
    pub updated_at: String,
    #[serde(default)]
    pub skills: HashMap<String, ManifestSkillEntry>,
    #[serde(default)]
    pub agents: HashMap<String, ManifestAgentMeta>,
}

/// 清单中的单个 skill 条目；key = `normalize(name)`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSkillEntry {
    /// 展示名（manifest name），与 key 的关系为 `normalize(name)`。
    pub name: String,
    /// 中枢内目录名，供远端还原。
    pub dir_name: String,
    /// 明文 zip 的 sha256（身份哈希）。
    pub hash: String,
    /// `blobs/<hash>.bin`。
    pub blob: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub installed_at: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub no_full_coverage: bool,
    /// 墓碑：true 表示该 skill 已被删除。
    #[serde(default)]
    pub deleted: bool,
    pub updated_at: String,
}

/// agent 便携元数据；`portableKey` 为内置 `AgentType::as_str()` 或 custom 名称。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestAgentMeta {
    #[serde(default)]
    pub user_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SyncConflictKind {
    /// 两端内容都变了。
    BothModified,
    /// 一端删除、另一端修改。
    DeleteVsModify,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflict {
    /// `normalize(name)`。
    pub skill_id: String,
    pub name: String,
    pub dir_name: String,
    #[serde(default)]
    pub local_hash: Option<String>,
    #[serde(default)]
    pub remote_hash: Option<String>,
    pub remote_device_id: String,
    pub kind: SyncConflictKind,
    pub detected_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SyncConflictChoice {
    Local,
    Remote,
    /// 都留：远端改名保存。
    Rename,
}

fn default_region() -> String {
    "us-east-1".to_string()
}

fn default_poll_secs() -> u64 {
    60
}
