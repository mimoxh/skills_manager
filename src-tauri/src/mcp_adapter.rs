use crate::{
    error::{AppError, AppResult},
    models::{AgentMcpServer, AgentProfile, AgentType, McpServerConfig},
};
use std::path::{Path, PathBuf};

// ---- 共享文件读写（消除 3 个 JSON 适配器 + 1 个 TOML 适配器的 read/write 重复）----

/// 读取 JSON 配置文件，文件不存在时返回空对象。Claude / OpenCode / Trae 共用。
pub fn read_json_file(path: &Path) -> AppResult<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::Value::Object(Default::default()));
    }
    let text = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    Ok(value)
}

/// 写入 JSON 配置文件，自动创建父目录。
pub fn write_json_file(path: &Path, value: &serde_json::Value) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    std::fs::write(path, text)?;
    Ok(())
}

/// 读取 TOML 配置文件，文件不存在时返回空表。Codex 专用。
pub fn read_toml_file(path: &Path) -> AppResult<toml::Value> {
    if !path.exists() {
        return Ok(toml::Value::Table(Default::default()));
    }
    let text = std::fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&text)?;
    Ok(value)
}

/// 写入 TOML 配置文件，自动创建父目录。
pub fn write_toml_file(path: &Path, value: &toml::Value) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(value)
        .map_err(|e| AppError::Message(format!("TOML 序列化失败: {}", e)))?;
    std::fs::write(path, text)?;
    Ok(())
}

// ---- MCP 段存取（消除 4 个适配器 add/update/remove/toggle 中 ×16 的 get-section 样板）----

/// 从 JSON 根对象中获取指定 key 的 MCP 服务器对象（可变引用）。
/// `agent_label` 用于错误消息（如 "Claude" / "Codex" / "Trae"）。
pub fn json_mcp_section_mut<'a>(
    root: &'a mut serde_json::Value,
    key: &str,
    agent_label: &str,
) -> AppResult<&'a mut serde_json::Map<String, serde_json::Value>> {
    root.as_object_mut()
        .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
        .get_mut(key)
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| {
            AppError::Message(format!("未找到 {} 的 {} 配置", agent_label, key))
        })
}

/// 确保 JSON 根对象中存在指定 key 的 MCP 服务器对象（不存在则创建空表）。
pub fn ensure_json_mcp_section(root: &mut serde_json::Value, key: &str) -> AppResult<()> {
    if root.get(key).is_none() {
        root.as_object_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .insert(
                key.to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
    }
    Ok(())
}

/// 从 TOML 根表中获取指定 key 的 MCP 服务器表（可变引用）。
pub fn toml_mcp_section_mut<'a>(
    root: &'a mut toml::Value,
    key: &str,
    agent_label: &str,
) -> AppResult<&'a mut toml::map::Map<String, toml::Value>> {
    root.as_table_mut()
        .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
        .get_mut(key)
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| {
            AppError::Message(format!("未找到 {} 的 {} 配置", agent_label, key))
        })
}

/// 确保 TOML 根表中存在指定 key 的 MCP 服务器表（不存在则创建空表）。
pub fn ensure_toml_mcp_section(root: &mut toml::Value, key: &str) -> AppResult<()> {
    if root.get(key).is_none() {
        root.as_table_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .insert(
                key.to_string(),
                toml::Value::Table(toml::map::Map::new()),
            );
    }
    Ok(())
}

// ---- 备份收敛（消除 4 个适配器 21 行重复的 backup 实现）----

/// 备份配置文件到统一备份根目录。
/// `prefix` 区分来源（如 "claude-config"），`ext` 指定扩展名（"json"/"toml"）。
pub fn backup_config_file(
    config_path: &Path,
    backup_root: &Path,
    prefix: &str,
    ext: &str,
    agent_label: &str,
) -> AppResult<PathBuf> {
    if !config_path.exists() {
        return Err(AppError::Message(format!(
            "{} 配置文件不存在",
            agent_label
        )));
    }
    let backup_name = format!(
        "{}-{}.{}",
        prefix,
        chrono::Utc::now().format("%Y%m%d%H%M%S%3f"),
        ext
    );
    let backup_path = backup_root.join(backup_name);
    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(config_path, &backup_path)?;
    Ok(backup_path)
}

// ---- config_path 自定��路径解析（消除 4 个适配器 config_path 中的样板）----

/// 解析 MCP 配置文件路径：优先读取 `adapter_config.mcpConfigPath`（经扩展名和父目录校验），
/// 其次回退到 `default_path`。
pub fn resolve_mcp_config_path(
    adapter_config: &Option<serde_json::Value>,
    expected_extensions: &[&str],
    default_path: Option<PathBuf>,
    agent_label: &str,
) -> AppResult<PathBuf> {
    if let Some(config) = adapter_config {
        if let Some(path) = config.get("mcpConfigPath").and_then(|v| v.as_str()) {
            if let Some(custom) = resolve_custom_config_path(path, expected_extensions)? {
                return Ok(custom);
            }
        }
    }
    default_path
        .ok_or_else(|| AppError::Message(format!("无法确定 {} 配置路径", agent_label)))
}

// ---- L-M7: mcpFormat 字符串到 AgentType 的映射 ----

/// 将 adapterConfig.mcpFormat 映射到对应的 AgentType，用于路由 MCP 适配器。
pub fn mcp_format_to_agent_type(format: &str) -> Option<AgentType> {
    match format {
        "claude" | "generic" => Some(AgentType::ClaudeCode),
        "opencode" => Some(AgentType::OpenCode),
        "codex" => Some(AgentType::Codex),
        "trae" => Some(AgentType::Trae),
        _ => None,
    }
}

/// 校验并解析用户自定义的 `mcpConfigPath`（来自 adapterConfig）。
/// 仅接受扩展名匹配且父目录存在的路径，防止配置被指向任意文件造成任意读写。
pub fn resolve_custom_config_path(
    input: &str,
    expected_extensions: &[&str],
) -> AppResult<Option<PathBuf>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(trimmed);
    let ext_ok = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            expected_extensions
                .iter()
                .any(|expected| ext.eq_ignore_ascii_case(expected))
        })
        .unwrap_or(false);
    if !ext_ok {
        return Err(AppError::Message(format!(
            "mcpConfigPath 必须指向 {} 文件: {}",
            expected_extensions.join(" / "),
            trimmed
        )));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err(AppError::Message(format!(
                "mcpConfigPath 的父目录不存在: {}",
                trimmed
            )));
        }
    }
    Ok(Some(path))
}

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

    /// 备份配置文件到统一备份根目录，返回备份路径
    fn backup(&self, profile: &AgentProfile, backup_root: &Path) -> AppResult<PathBuf>;

    /// 配置文件路径
    fn config_path(&self, profile: &AgentProfile) -> AppResult<PathBuf>;
}
