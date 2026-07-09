use crate::{
    error::{AppError, AppResult},
    mcp_adapter::McpAdapter,
    mcp_claude::ClaudeMcpAdapter,
    mcp_codex::CodexMcpAdapter,
    mcp_opencode::OpenCodeMcpAdapter,
    mcp_trae::TraeMcpAdapter,
    models::{
        AgentMcpServer, AgentProfile, AgentType, ConflictPolicy, GroupedMcpServer,
        McpOperationResult, McpServerConfig,
    },
    store::AppStore,
};
use std::{collections::HashMap, sync::Arc};

pub struct McpService {
    adapters: HashMap<AgentType, Box<dyn McpAdapter + Send + Sync>>,
}

impl McpService {
    pub fn new(_store: Arc<AppStore>) -> Self {
        let mut adapters: HashMap<AgentType, Box<dyn McpAdapter + Send + Sync>> = HashMap::new();
        adapters.insert(AgentType::Codex, Box::new(CodexMcpAdapter::new()));
        adapters.insert(AgentType::ClaudeCode, Box::new(ClaudeMcpAdapter::new()));
        adapters.insert(AgentType::OpenCode, Box::new(OpenCodeMcpAdapter::new()));
        adapters.insert(AgentType::Trae, Box::new(TraeMcpAdapter::new()));
        Self { adapters }
    }

    fn get_adapter(&self, agent_type: &AgentType) -> Option<&(dyn McpAdapter + Send + Sync)> {
        self.adapters.get(agent_type).map(|a| a.as_ref())
    }

    /// 为 Custom 类型 agent 根据 adapterConfig.mcpFormat 路由到对应适配器
    fn get_adapter_for_agent(&self, agent: &AgentProfile) -> Option<&(dyn McpAdapter + Send + Sync)> {
        if agent.agent_type == AgentType::Custom {
            let format = agent.adapter_config.as_ref()
                .and_then(|c| c.get("mcpFormat"))
                .and_then(|v| v.as_str());
            match format {
                Some("claude") | Some("generic") => self.adapters.get(&AgentType::ClaudeCode).map(|a| a.as_ref()),
                Some("opencode") => self.adapters.get(&AgentType::OpenCode).map(|a| a.as_ref()),
                Some("codex") => self.adapters.get(&AgentType::Codex).map(|a| a.as_ref()),
                Some("trae") => self.adapters.get(&AgentType::Trae).map(|a| a.as_ref()),
                _ => None,
            }
        } else {
            self.get_adapter(&agent.agent_type)
        }
    }

    /// 扫描所有 Agent 的 MCP server，按名称分组
    pub fn scan_mcp_servers(
        &self,
        agents: &[AgentProfile],
    ) -> AppResult<Vec<GroupedMcpServer>> {
        let mut all_servers: Vec<AgentMcpServer> = Vec::new();

        for agent in agents {
            if let Some(adapter) = self.get_adapter_for_agent(agent) {
                match adapter.scan(agent) {
                    Ok(servers) => all_servers.extend(servers),
                    Err(_) => {
                        // 某个 agent 扫描失败时跳过，不影响其他 agent
                        continue;
                    }
                }
            }
        }

        // 按 server name 分组
        let mut grouped: HashMap<String, Vec<AgentMcpServer>> = HashMap::new();
        for server in all_servers {
            grouped
                .entry(server.config.name.clone())
                .or_default()
                .push(server);
        }

        let mut result: Vec<GroupedMcpServer> = grouped
            .into_iter()
            .map(|(name, copies)| {
                let agent_ids: Vec<String> = copies.iter().map(|c| c.agent_id.clone()).collect();
                let disabled_agent_ids: Vec<String> = copies
                    .iter()
                    .filter(|c| c.config.disabled)
                    .map(|c| c.agent_id.clone())
                    .collect();
                GroupedMcpServer {
                    name,
                    copies,
                    agent_ids,
                    disabled_agent_ids,
                }
            })
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(result)
    }

    /// 添加 MCP server 到指定的 agents
    pub fn add_mcp_server(
        &self,
        agents: &[AgentProfile],
        agent_ids: &[String],
        config: &McpServerConfig,
        conflict_policy: ConflictPolicy,
    ) -> AppResult<Vec<McpOperationResult>> {
        let agent_map: HashMap<_, _> = agents
            .iter()
            .map(|a| (a.id.clone(), a))
            .collect();

        let mut results = Vec::new();

        for agent_id in agent_ids {
            let agent = match agent_map.get(agent_id) {
                Some(a) => a,
                None => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: config.name.clone(),
                        action: "error".to_string(),
                        message: format!("找不到 Agent: {}", agent_id),
                    });
                    continue;
                }
            };

            let adapter = match self.get_adapter_for_agent(agent) {
                Some(a) => a,
                None => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: config.name.clone(),
                        action: "skipped".to_string(),
                        message: format!("{} 不支持 MCP 管理，已跳过", agent.name),
                    });
                    continue;
                }
            };

            // 检查是否已存在
            let existing = match adapter.scan(agent) {
                Ok(servers) => servers,
                Err(e) => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: config.name.clone(),
                        action: "error".to_string(),
                        message: format!("扫描 {} 的 MCP 配置失败: {}", agent.name, e),
                    });
                    continue;
                }
            };
            let exists = existing.iter().any(|s| s.config.name == config.name);

            if exists {
                match conflict_policy {
                    ConflictPolicy::Skip => {
                        results.push(McpOperationResult {
                            agent_id: agent_id.clone(),
                            server_name: config.name.clone(),
                            action: "skipped".to_string(),
                            message: format!("{} 已存在于 {}", config.name, agent.name),
                        });
                        continue;
                    }
                    ConflictPolicy::BackupOverwrite => {
                        let _ = adapter.backup(agent);
                        if let Err(e) = adapter.update(agent, &config.name, config) {
                            results.push(McpOperationResult {
                                agent_id: agent_id.clone(),
                                server_name: config.name.clone(),
                                action: "error".to_string(),
                                message: format!("更新 {} 于 {} 失败: {}", config.name, agent.name, e),
                            });
                        } else {
                            results.push(McpOperationResult {
                                agent_id: agent_id.clone(),
                                server_name: config.name.clone(),
                                action: "updated".to_string(),
                                message: format!("{} 已更新于 {}", config.name, agent.name),
                            });
                        }
                        continue;
                    }
                    ConflictPolicy::Prompt => {
                        return Err(AppError::Message(format!(
                            "MCP server '{}' 已存在于 {}。请先选择冲突策略。",
                            config.name, agent.name
                        )));
                    }
                    ConflictPolicy::Rename => {
                        // 不适用，直接覆盖
                        if let Err(e) = adapter.update(agent, &config.name, config) {
                            results.push(McpOperationResult {
                                agent_id: agent_id.clone(),
                                server_name: config.name.clone(),
                                action: "error".to_string(),
                                message: format!("更新 {} 于 {} 失败: {}", config.name, agent.name, e),
                            });
                        } else {
                            results.push(McpOperationResult {
                                agent_id: agent_id.clone(),
                                server_name: config.name.clone(),
                                action: "updated".to_string(),
                                message: format!("{} 已更新于 {}", config.name, agent.name),
                            });
                        }
                        continue;
                    }
                }
            }

            match adapter.add(agent, config) {
                Ok(()) => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: config.name.clone(),
                        action: "added".to_string(),
                        message: format!("{} 已添加到 {}", config.name, agent.name),
                    });
                }
                Err(e) => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: config.name.clone(),
                        action: "error".to_string(),
                        message: format!("添加 {} 到 {} 失败: {}", config.name, agent.name, e),
                    });
                }
            }
        }

        Ok(results)
    }

    /// 更新 MCP server
    pub fn update_mcp_server(
        &self,
        agents: &[AgentProfile],
        agent_id: &str,
        original_name: &str,
        config: &McpServerConfig,
    ) -> AppResult<McpOperationResult> {
        let agent = agents.iter().find(|a| a.id == agent_id).ok_or_else(|| {
            AppError::Message(format!("找不到 Agent: {}", agent_id))
        })?;

        let adapter = self.get_adapter_for_agent(agent).ok_or_else(|| {
            AppError::Message(format!("Agent '{}' 不支持 MCP 管理", agent.name))
        })?;

        adapter.backup(agent)?;
        adapter.update(agent, original_name, config)?;

        Ok(McpOperationResult {
            agent_id: agent_id.to_string(),
            server_name: config.name.clone(),
            action: "updated".to_string(),
            message: format!("{} 已更新于 {}", config.name, agent.name),
        })
    }

    /// 删除 MCP server
    pub fn remove_mcp_server(
        &self,
        agents: &[AgentProfile],
        agent_id: &str,
        name: &str,
    ) -> AppResult<McpOperationResult> {
        let agent = agents.iter().find(|a| a.id == agent_id).ok_or_else(|| {
            AppError::Message(format!("找不到 Agent: {}", agent_id))
        })?;

        let adapter = self.get_adapter_for_agent(agent).ok_or_else(|| {
            AppError::Message(format!("Agent '{}' 不支持 MCP 管理", agent.name))
        })?;

        adapter.backup(agent)?;
        adapter.remove(agent, name)?;

        Ok(McpOperationResult {
            agent_id: agent_id.to_string(),
            server_name: name.to_string(),
            action: "removed".to_string(),
            message: format!("{} 已从 {} 删除", name, agent.name),
        })
    }

    /// 禁用/启用 MCP server
    pub fn toggle_mcp_server(
        &self,
        agents: &[AgentProfile],
        agent_id: &str,
        name: &str,
        disabled: bool,
    ) -> AppResult<McpOperationResult> {
        let agent = agents.iter().find(|a| a.id == agent_id).ok_or_else(|| {
            AppError::Message(format!("找不到 Agent: {}", agent_id))
        })?;

        let adapter = self.get_adapter_for_agent(agent).ok_or_else(|| {
            AppError::Message(format!("Agent '{}' 不支持 MCP 管理", agent.name))
        })?;

        adapter.toggle(agent, name, disabled)?;

        let action = if disabled { "disabled" } else { "enabled" };
        Ok(McpOperationResult {
            agent_id: agent_id.to_string(),
            server_name: name.to_string(),
            action: action.to_string(),
            message: format!("{} 已{}于 {}", name, if disabled { "禁用" } else { "启用" }, agent.name),
        })
    }

    /// 同步 MCP server：从源 Agent 读取配置，写入到目标 Agents
    pub fn sync_mcp_server(
        &self,
        agents: &[AgentProfile],
        server_name: &str,
        source_agent_id: &str,
        target_agent_ids: &[String],
        conflict_policy: ConflictPolicy,
    ) -> AppResult<Vec<McpOperationResult>> {
        // 从源 Agent 扫描获取配置
        let source_agent = agents.iter().find(|a| a.id == source_agent_id).ok_or_else(|| {
            AppError::Message(format!("找不到源 Agent: {}", source_agent_id))
        })?;
        let source_adapter = self.get_adapter_for_agent(source_agent).ok_or_else(|| {
            AppError::Message(format!("源 Agent '{}' 不支持 MCP 管理", source_agent.name))
        })?;
        let source_servers = source_adapter.scan(source_agent)?;
        let source_server = source_servers.iter().find(|s| s.config.name == server_name).ok_or_else(|| {
            AppError::Message(format!("源 Agent '{}' 中未找到 MCP server '{}'", source_agent.name, server_name))
        })?;
        let config = &source_server.config;

        // 对每个目标 Agent 执行同步
        let agent_map: HashMap<_, _> = agents.iter().map(|a| (a.id.clone(), a)).collect();
        let mut results = Vec::new();

        for agent_id in target_agent_ids {
            let agent = match agent_map.get(agent_id) {
                Some(a) => a,
                None => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: server_name.to_string(),
                        action: "error".to_string(),
                        message: format!("找不到 Agent: {}", agent_id),
                    });
                    continue;
                }
            };
            let adapter = match self.get_adapter_for_agent(agent) {
                Some(a) => a,
                None => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: server_name.to_string(),
                        action: "skipped".to_string(),
                        message: format!("{} 不支持 MCP 管理，已跳过", agent.name),
                    });
                    continue;
                }
            };

            let existing = match adapter.scan(agent) {
                Ok(servers) => servers,
                Err(e) => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: server_name.to_string(),
                        action: "error".to_string(),
                        message: format!("扫描 {} 的 MCP 配置失败: {}", agent.name, e),
                    });
                    continue;
                }
            };
            let exists = existing.iter().any(|s| s.config.name == server_name);

            if exists {
                match conflict_policy {
                    ConflictPolicy::Skip => {
                        results.push(McpOperationResult {
                            agent_id: agent_id.clone(),
                            server_name: server_name.to_string(),
                            action: "skipped".to_string(),
                            message: format!("{} 已存在于 {}", server_name, agent.name),
                        });
                        continue;
                    }
                    ConflictPolicy::BackupOverwrite => {
                        let _ = adapter.backup(agent);
                        if let Err(e) = adapter.update(agent, server_name, config) {
                            results.push(McpOperationResult {
                                agent_id: agent_id.clone(),
                                server_name: server_name.to_string(),
                                action: "error".to_string(),
                                message: format!("更新 {} 于 {} 失败: {}", server_name, agent.name, e),
                            });
                        } else {
                            results.push(McpOperationResult {
                                agent_id: agent_id.clone(),
                                server_name: server_name.to_string(),
                                action: "updated".to_string(),
                                message: format!("{} 已更新于 {}", server_name, agent.name),
                            });
                        }
                        continue;
                    }
                    ConflictPolicy::Prompt => {
                        return Err(AppError::Message(format!(
                            "MCP server '{}' 已存在于 {}。请先选择冲突策略。",
                            server_name, agent.name
                        )));
                    }
                    ConflictPolicy::Rename => {
                        if let Err(e) = adapter.update(agent, server_name, config) {
                            results.push(McpOperationResult {
                                agent_id: agent_id.clone(),
                                server_name: server_name.to_string(),
                                action: "error".to_string(),
                                message: format!("更新 {} 于 {} 失败: {}", server_name, agent.name, e),
                            });
                        } else {
                            results.push(McpOperationResult {
                                agent_id: agent_id.clone(),
                                server_name: server_name.to_string(),
                                action: "updated".to_string(),
                                message: format!("{} 已更新于 {}", server_name, agent.name),
                            });
                        }
                        continue;
                    }
                }
            }

            match adapter.add(agent, config) {
                Ok(()) => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: server_name.to_string(),
                        action: "added".to_string(),
                        message: format!("{} 已添加到 {}", server_name, agent.name),
                    });
                }
                Err(e) => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: server_name.to_string(),
                        action: "error".to_string(),
                        message: format!("添加 {} 到 {} 失败: {}", server_name, agent.name, e),
                    });
                }
            }
        }

        Ok(results)
    }

    /// 批量从多个 Agent 删除 MCP server
    pub fn remove_mcp_server_from_agents(
        &self,
        agents: &[AgentProfile],
        server_name: &str,
        agent_ids: &[String],
    ) -> AppResult<Vec<McpOperationResult>> {
        let agent_map: HashMap<_, _> = agents.iter().map(|a| (a.id.clone(), a)).collect();
        let mut results = Vec::new();

        for agent_id in agent_ids {
            let agent = agent_map.get(agent_id).ok_or_else(|| {
                AppError::Message(format!("找不到 Agent: {}", agent_id))
            })?;
            let adapter = match self.get_adapter_for_agent(agent) {
                Some(a) => a,
                None => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: server_name.to_string(),
                        action: "skipped".to_string(),
                        message: format!("{} 不支持 MCP 管理，已跳过", agent.name),
                    });
                    continue;
                }
            };

            // backup 或 remove 失败时记录错误并继续，不中断批量操作
            if let Err(e) = adapter.backup(agent) {
                results.push(McpOperationResult {
                    agent_id: agent_id.clone(),
                    server_name: server_name.to_string(),
                    action: "error".to_string(),
                    message: format!("备份 {} 的配置失败: {}", agent.name, e),
                });
                continue;
            }
            match adapter.remove(agent, server_name) {
                Ok(()) => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: server_name.to_string(),
                        action: "removed".to_string(),
                        message: format!("{} 已从 {} 删除", server_name, agent.name),
                    });
                }
                Err(e) => {
                    results.push(McpOperationResult {
                        agent_id: agent_id.clone(),
                        server_name: server_name.to_string(),
                        action: "error".to_string(),
                        message: format!("从 {} 删除 {} 失败: {}", agent.name, server_name, e),
                    });
                }
            }
        }

        Ok(results)
    }
}
