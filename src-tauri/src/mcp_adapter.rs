use crate::{
    error::AppResult,
    models::{AgentMcpServer, AgentProfile, McpServerConfig},
};
use std::path::PathBuf;

/// MCP 适配器接口，每个 Agent 类型实现一个
pub trait McpAdapter {
    /// 扫描该 Agent 的 MCP server 配置
    fn scan(&self, profile: &AgentProfile) -> AppResult<Vec<AgentMcpServer>>;

    /// 添加一个新的 MCP server
    fn add(&self, profile: &AgentProfile, config: &McpServerConfig) -> AppResult<()>;

    /// 更新一个已有的 MCP server
    fn update(
        &self,
        profile: &AgentProfile,
        original_name: &str,
        config: &McpServerConfig,
    ) -> AppResult<()>;

    /// 删除一个 MCP server
    fn remove(&self, profile: &AgentProfile, name: &str) -> AppResult<()>;

    /// 禁用/启用一个 MCP server
    fn toggle(&self, profile: &AgentProfile, name: &str, disabled: bool) -> AppResult<()>;

    /// 备份配置文件，返回备份路径
    fn backup(&self, profile: &AgentProfile) -> AppResult<PathBuf>;

    /// 配置文件路径
    fn config_path(&self, profile: &AgentProfile) -> AppResult<PathBuf>;
}
