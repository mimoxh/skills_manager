use crate::{
    error::{AppError, AppResult},
    mcp_adapter::McpAdapter,
    models::{AgentMcpServer, AgentProfile, McpServerConfig, McpTransport},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Codex MCP 适配器，读写 `%USERPROFILE%\.codex\config.toml`
pub struct CodexMcpAdapter;

impl CodexMcpAdapter {
    pub fn new() -> Self {
        Self
    }

    fn codex_config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".codex").join("config.toml"))
    }

    fn read_config(path: &Path) -> AppResult<toml::Value> {
        if !path.exists() {
            return Ok(toml::Value::Table(Default::default()));
        }
        let text = fs::read_to_string(path)?;
        let value: toml::Value = toml::from_str(&text)?;
        Ok(value)
    }

    fn write_config(path: &Path, config: &toml::Value) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(config)
            .map_err(|e| AppError::Message(format!("TOML 序列化失败: {}", e)))?;
        fs::write(path, text)?;
        Ok(())
    }

    fn parse_mcp_server(
        name: &str,
        table: &toml::map::Map<String, toml::Value>,
    ) -> McpServerConfig {
        let command = table
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let url = table
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let transport = if url.is_some() {
            // 检查是否指定了 sse transport
            if table
                .get("transport")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .as_deref()
                == Some("sse")
            {
                McpTransport::Sse
            } else {
                McpTransport::Http
            }
        } else {
            McpTransport::Stdio
        };

        let args = table
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let env = table
            .get("env")
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let headers = table
            .get("headers")
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let disabled = table
            .get("disabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let timeout_sec = table
            .get("timeout")
            .and_then(|v| v.as_integer())
            .map(|n| n as u64);

        McpServerConfig {
            name: name.to_string(),
            transport,
            command,
            args,
            env,
            url,
            headers,
            disabled,
            timeout_sec,
        }
    }

    fn config_to_toml_table(config: &McpServerConfig) -> toml::Value {
        let mut table = toml::map::Map::new();

        match config.transport {
            McpTransport::Stdio => {
                if let Some(ref cmd) = config.command {
                    table.insert("command".to_string(), toml::Value::String(cmd.clone()));
                }
                if !config.args.is_empty() {
                    let args: Vec<toml::Value> = config
                        .args
                        .iter()
                        .map(|a| toml::Value::String(a.clone()))
                        .collect();
                    table.insert("args".to_string(), toml::Value::Array(args));
                }
                if !config.env.is_empty() {
                    let env: toml::map::Map<String, toml::Value> = config
                        .env
                        .iter()
                        .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
                        .collect();
                    table.insert("env".to_string(), toml::Value::Table(env));
                }
            }
            McpTransport::Http | McpTransport::Sse => {
                if let Some(ref url) = config.url {
                    table.insert("url".to_string(), toml::Value::String(url.clone()));
                }
                if config.transport == McpTransport::Sse {
                    table.insert(
                        "transport".to_string(),
                        toml::Value::String("sse".to_string()),
                    );
                }
                if !config.headers.is_empty() {
                    let headers: toml::map::Map<String, toml::Value> = config
                        .headers
                        .iter()
                        .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
                        .collect();
                    table.insert("headers".to_string(), toml::Value::Table(headers));
                }
            }
        }

        if config.disabled {
            table.insert("disabled".to_string(), toml::Value::Boolean(true));
        }
        if let Some(timeout) = config.timeout_sec {
            table.insert(
                "timeout".to_string(),
                toml::Value::Integer(timeout as i64),
            );
        }

        toml::Value::Table(table)
    }
}

impl McpAdapter for CodexMcpAdapter {
    fn scan(&self, profile: &AgentProfile) -> AppResult<Vec<AgentMcpServer>> {
        let path = self.config_path(profile)?;
        let config = Self::read_config(&path)?;
        let mut servers = Vec::new();

        if let Some(mcp_table) = config.get("mcp_servers").and_then(|v| v.as_table()) {
            for (name, value) in mcp_table {
                if let Some(table) = value.as_table() {
                    let server_config = Self::parse_mcp_server(name, table);
                    // 包装为 [mcp_servers.name] 完整结构
                    let mut root_table = toml::map::Map::new();
                    let mut mcp_servers = toml::map::Map::new();
                    mcp_servers.insert(name.clone(), value.clone());
                    root_table.insert("mcp_servers".to_string(), toml::Value::Table(mcp_servers));
                    let raw = toml::to_string_pretty(&toml::Value::Table(root_table)).ok();
                    servers.push(AgentMcpServer {
                        agent_id: profile.id.clone(),
                        agent_name: profile.name.clone(),
                        config_path: path.to_string_lossy().to_string(),
                        fingerprint: format!("{:x}", md5_hash(&format!("{:?}", server_config))),
                        config: server_config,
                        raw_config: raw,
                    });
                }
            }
        }

        Ok(servers)
    }

    fn add(&self, profile: &AgentProfile, config: &McpServerConfig) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = Self::read_config(&path)?;

        // 确保 mcp_servers 表存在
        if root.get("mcp_servers").is_none() {
            root.as_table_mut().unwrap().insert(
                "mcp_servers".to_string(),
                toml::Value::Table(toml::map::Map::new()),
            );
        }

        let mcp_servers = root
            .as_table_mut()
            .unwrap()
            .get_mut("mcp_servers")
            .unwrap()
            .as_table_mut()
            .unwrap();

        if mcp_servers.contains_key(&config.name) {
            return Err(AppError::Message(format!(
                "MCP server '{}' 已存在于 Codex 配置中",
                config.name
            )));
        }

        mcp_servers.insert(
            config.name.clone(),
            Self::config_to_toml_table(config),
        );

        Self::write_config(&path, &root)
    }

    fn update(
        &self,
        profile: &AgentProfile,
        original_name: &str,
        config: &McpServerConfig,
    ) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = Self::read_config(&path)?;

        let mcp_servers = root
            .as_table_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .get_mut("mcp_servers")
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| AppError::Message("未找到 mcp_servers 配置".to_string()))?;

        if !mcp_servers.contains_key(original_name) {
            return Err(AppError::Message(format!(
                "MCP server '{}' 不存在于 Codex 配置中",
                original_name
            )));
        }

        // 如果名称变了，删除旧的
        if original_name != config.name {
            mcp_servers.remove(original_name);
        }

        mcp_servers.insert(config.name.clone(), Self::config_to_toml_table(config));

        Self::write_config(&path, &root)
    }

    fn remove(&self, profile: &AgentProfile, name: &str) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = Self::read_config(&path)?;

        let mcp_servers = root
            .as_table_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .get_mut("mcp_servers")
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| AppError::Message("未找到 mcp_servers 配置".to_string()))?;

        if mcp_servers.remove(name).is_none() {
            return Err(AppError::Message(format!(
                "MCP server '{}' 不存在于 Codex 配置中",
                name
            )));
        }

        Self::write_config(&path, &root)
    }

    fn toggle(&self, profile: &AgentProfile, name: &str, disabled: bool) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = Self::read_config(&path)?;

        let mcp_servers = root
            .as_table_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .get_mut("mcp_servers")
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| AppError::Message("未找到 mcp_servers 配置".to_string()))?;

        let server_table = mcp_servers
            .get_mut(name)
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| AppError::Message(format!("MCP server '{}' 不存在", name)))?;

        if disabled {
            server_table.insert("disabled".to_string(), toml::Value::Boolean(true));
        } else {
            server_table.remove("disabled");
        }

        Self::write_config(&path, &root)
    }

    fn backup(&self, profile: &AgentProfile) -> AppResult<PathBuf> {
        let path = self.config_path(profile)?;
        if !path.exists() {
            return Err(AppError::Message("Codex 配置文件不存在".to_string()));
        }
        let backup_name = format!(
            "codex-config-{}.toml",
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        );
        let backup_path = Path::new(&profile.skills_path)
            .parent()
            .unwrap_or(Path::new("."))
            .join("backups")
            .join(backup_name);
        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&path, &backup_path)?;
        Ok(backup_path)
    }

    fn config_path(&self, profile: &AgentProfile) -> AppResult<PathBuf> {
        // 优先使用 adapter_config 中的自定义路径
        if let Some(config) = &profile.adapter_config {
            if let Some(path) = config.get("mcpConfigPath").and_then(|v| v.as_str()) {
                let trimmed = path.trim();
                if !trimmed.is_empty() {
                    return Ok(PathBuf::from(trimmed));
                }
            }
        }
        Self::codex_config_path()
            .ok_or_else(|| AppError::Message("无法确定 Codex 配置路径".to_string()))
    }
}

/// 简单的 MD5 哈希（用于 fingerprint，非密码学用途）
fn md5_hash(input: &str) -> u128 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish() as u128
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stdio_mcp_from_toml() {
        let toml_str = r#"
[mcp_servers.example]
command = "npx"
args = ["-y", "some-mcp-server"]

[mcp_servers.example.env]
API_KEY = "test-key"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let table = value
            .get("mcp_servers")
            .and_then(|v| v.as_table())
            .unwrap();
        let server = CodexMcpAdapter::parse_mcp_server("example", table.get("example").unwrap().as_table().unwrap());

        assert_eq!(server.name, "example");
        assert_eq!(server.transport, McpTransport::Stdio);
        assert_eq!(server.command.as_deref(), Some("npx"));
        assert_eq!(server.args, vec!["-y", "some-mcp-server"]);
        assert_eq!(server.env.get("API_KEY").map(|s| s.as_str()), Some("test-key"));
    }

    #[test]
    fn parses_http_mcp_from_toml() {
        let toml_str = r#"
[mcp_servers.docs]
url = "https://example.com/mcp"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let table = value
            .get("mcp_servers")
            .and_then(|v| v.as_table())
            .unwrap();
        let server = CodexMcpAdapter::parse_mcp_server("docs", table.get("docs").unwrap().as_table().unwrap());

        assert_eq!(server.name, "docs");
        assert_eq!(server.transport, McpTransport::Http);
        assert_eq!(server.url.as_deref(), Some("https://example.com/mcp"));
    }

    #[test]
    fn parses_sse_mcp_from_toml() {
        let toml_str = r#"
[mcp_servers.stream]
url = "https://example.com/sse"
transport = "sse"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let table = value
            .get("mcp_servers")
            .and_then(|v| v.as_table())
            .unwrap();
        let server = CodexMcpAdapter::parse_mcp_server("stream", table.get("stream").unwrap().as_table().unwrap());

        assert_eq!(server.transport, McpTransport::Sse);
    }

    #[test]
    fn parses_disabled_mcp() {
        let toml_str = r#"
[mcp_servers.old]
command = "npx"
disabled = true
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let table = value
            .get("mcp_servers")
            .and_then(|v| v.as_table())
            .unwrap();
        let server = CodexMcpAdapter::parse_mcp_server("old", table.get("old").unwrap().as_table().unwrap());

        assert!(server.disabled);
    }
}
