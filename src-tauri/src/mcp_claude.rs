use crate::{
    error::{AppError, AppResult},
    mcp_adapter::{self, McpAdapter},
    models::{AgentMcpServer, AgentProfile, McpServerConfig, McpTransport},
};
use serde_json::Value as JsonValue;
use std::{
    path::{Path, PathBuf},
};

/// Claude Code MCP 适配器，读写 `~/.claude.json`
const AGENT_LABEL: &str = "Claude";
const SECTION_KEY: &str = "mcpServers";

pub struct ClaudeMcpAdapter;

impl ClaudeMcpAdapter {
    pub fn new() -> Self {
        Self
    }

    /// 获取默认配置文件路径
    fn default_config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".claude.json"))
    }

    fn parse_mcp_server(name: &str, obj: &serde_json::Map<String, JsonValue>) -> McpServerConfig {
        let command = obj
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let url = obj
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let transport = if url.is_some() {
            let type_field = obj
                .get("type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase());
            let transport_field = obj
                .get("transport")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase());

            if transport_field.as_deref() == Some("sse") || type_field.as_deref() == Some("sse") {
                McpTransport::Sse
            } else {
                McpTransport::Http
            }
        } else {
            McpTransport::Stdio
        };

        let args = obj
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let env = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let headers = obj
            .get("headers")
            .and_then(|v| v.as_object())
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let disabled = obj
            .get("disabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let timeout_sec = obj
            .get("timeout")
            .and_then(|v| v.as_u64());

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

    fn config_to_json_object(config: &McpServerConfig) -> JsonValue {
        let mut obj = serde_json::Map::new();

        match config.transport {
            McpTransport::Stdio => {
                if let Some(ref cmd) = config.command {
                    obj.insert("command".to_string(), JsonValue::String(cmd.clone()));
                }
                if !config.args.is_empty() {
                    let args: Vec<JsonValue> = config
                        .args
                        .iter()
                        .map(|a| JsonValue::String(a.clone()))
                        .collect();
                    obj.insert("args".to_string(), JsonValue::Array(args));
                }
                if !config.env.is_empty() {
                    let env: serde_json::Map<String, JsonValue> = config
                        .env
                        .iter()
                        .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
                        .collect();
                    obj.insert("env".to_string(), JsonValue::Object(env));
                }
            }
            McpTransport::Http | McpTransport::Sse => {
                if let Some(ref url) = config.url {
                    obj.insert("url".to_string(), JsonValue::String(url.clone()));
                }
                if config.transport == McpTransport::Sse {
                    obj.insert(
                        "transport".to_string(),
                        JsonValue::String("sse".to_string()),
                    );
                } else {
                    // Claude 要求 HTTP 类型必须声明 type 字段
                    obj.insert(
                        "type".to_string(),
                        JsonValue::String("streamable-http".to_string()),
                    );
                }
                if !config.headers.is_empty() {
                    let headers: serde_json::Map<String, JsonValue> = config
                        .headers
                        .iter()
                        .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
                        .collect();
                    obj.insert("headers".to_string(), JsonValue::Object(headers));
                }
            }
        }

        if config.disabled {
            obj.insert("disabled".to_string(), JsonValue::Bool(true));
        }
        if let Some(timeout) = config.timeout_sec {
            obj.insert("timeout".to_string(), JsonValue::Number(timeout.into()));
        }

        JsonValue::Object(obj)
    }
}

impl McpAdapter for ClaudeMcpAdapter {
    fn scan(&self, profile: &AgentProfile) -> AppResult<Vec<AgentMcpServer>> {
        let path = self.config_path(profile)?;
        let root = mcp_adapter::read_json_file(&path)?;
        let mut servers = Vec::new();

        if let Some(mcp_obj) = root.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, value) in mcp_obj {
                if let Some(obj) = value.as_object() {
                    let server_config = Self::parse_mcp_server(name, obj);
                    // 包装为 { "server_name": { ... } } 完整结构
                    let mut wrapper = serde_json::Map::new();
                    wrapper.insert(name.clone(), value.clone());
                    let raw = Some(serde_json::to_string_pretty(&wrapper)?);
                    servers.push(AgentMcpServer {
                        agent_id: profile.id.clone(),
                        agent_name: profile.name.clone(),
                        config_path: path.to_string_lossy().to_string(),
                        fingerprint: crate::hash::stable_fingerprint(&server_config)?,
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
        let mut root = mcp_adapter::read_json_file(&path)?;

        mcp_adapter::ensure_json_mcp_section(&mut root, SECTION_KEY)?;
        let mcp_servers = mcp_adapter::json_mcp_section_mut(&mut root, SECTION_KEY, AGENT_LABEL)?;

        if mcp_servers.contains_key(&config.name) {
            return Err(AppError::Message(format!(
                "MCP server '{}' 已存在于 Claude 配置中",
                config.name
            )));
        }

        mcp_servers.insert(config.name.clone(), Self::config_to_json_object(config));

        mcp_adapter::write_json_file(&path, &root)
    }

    fn update(
        &self,
        profile: &AgentProfile,
        original_name: &str,
        config: &McpServerConfig,
    ) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = mcp_adapter::read_json_file(&path)?;

        let mcp_servers = mcp_adapter::json_mcp_section_mut(&mut root, SECTION_KEY, AGENT_LABEL)?;

        if !mcp_servers.contains_key(original_name) {
            return Err(AppError::Message(format!(
                "MCP server '{}' 不存在于 Claude 配置中",
                original_name
            )));
        }

        // 如果名称变了，删除旧的
        if original_name != config.name {
            mcp_servers.remove(original_name);
        }

        mcp_servers.insert(config.name.clone(), Self::config_to_json_object(config));

        mcp_adapter::write_json_file(&path, &root)
    }

    fn remove(&self, profile: &AgentProfile, name: &str) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = mcp_adapter::read_json_file(&path)?;

        let mcp_servers = mcp_adapter::json_mcp_section_mut(&mut root, SECTION_KEY, AGENT_LABEL)?;

        if mcp_servers.remove(name).is_none() {
            return Err(AppError::Message(format!(
                "MCP server '{}' 不存在于 Claude 配置中",
                name
            )));
        }

        mcp_adapter::write_json_file(&path, &root)
    }

    fn toggle(&self, profile: &AgentProfile, name: &str, disabled: bool) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = mcp_adapter::read_json_file(&path)?;

        let mcp_servers = mcp_adapter::json_mcp_section_mut(&mut root, SECTION_KEY, AGENT_LABEL)?;

        let server_obj = mcp_servers
            .get_mut(name)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| AppError::Message(format!("MCP server '{}' 不存在", name)))?;

        if disabled {
            server_obj.insert("disabled".to_string(), JsonValue::Bool(true));
        } else {
            server_obj.remove("disabled");
        }

        mcp_adapter::write_json_file(&path, &root)
    }

    fn backup(&self, profile: &AgentProfile, backup_root: &Path) -> AppResult<PathBuf> {
        let path = self.config_path(profile)?;
        mcp_adapter::backup_config_file(&path, backup_root, "claude-config", "json", AGENT_LABEL)
    }

    fn config_path(&self, profile: &AgentProfile) -> AppResult<PathBuf> {
        mcp_adapter::resolve_mcp_config_path(
            &profile.adapter_config,
            &["json"],
            Self::default_config_path(),
            AGENT_LABEL,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stdio_mcp_from_json() {
        let json_str = r#"{
            "mcpServers": {
                "example": {
                    "command": "npx",
                    "args": ["-y", "some-mcp-server"],
                    "env": { "API_KEY": "test-key" }
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .unwrap();
        let server = ClaudeMcpAdapter::parse_mcp_server("example", obj.get("example").unwrap().as_object().unwrap());

        assert_eq!(server.name, "example");
        assert_eq!(server.transport, McpTransport::Stdio);
        assert_eq!(server.command.as_deref(), Some("npx"));
        assert_eq!(server.args, vec!["-y", "some-mcp-server"]);
        assert_eq!(server.env.get("API_KEY").map(|s| s.as_str()), Some("test-key"));
    }

    #[test]
    fn parses_http_mcp_from_json() {
        let json_str = r#"{
            "mcpServers": {
                "docs": {
                    "url": "https://example.com/mcp"
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .unwrap();
        let server = ClaudeMcpAdapter::parse_mcp_server("docs", obj.get("docs").unwrap().as_object().unwrap());

        assert_eq!(server.transport, McpTransport::Http);
        assert_eq!(server.url.as_deref(), Some("https://example.com/mcp"));
    }

    #[test]
    fn parses_disabled_mcp_from_json() {
        let json_str = r#"{
            "mcpServers": {
                "old": {
                    "command": "npx",
                    "disabled": true
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .unwrap();
        let server = ClaudeMcpAdapter::parse_mcp_server("old", obj.get("old").unwrap().as_object().unwrap());

        assert!(server.disabled);
    }

    #[test]
    fn parses_streamable_http_type() {
        let json_str = r#"{
            "mcpServers": {
                "affine": {
                    "type": "streamable-http",
                    "url": "https://example.com/mcp",
                    "headers": {
                        "Authorization": "Bearer token123"
                    }
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .unwrap();
        let server = ClaudeMcpAdapter::parse_mcp_server("affine", obj.get("affine").unwrap().as_object().unwrap());

        assert_eq!(server.transport, McpTransport::Http);
        assert_eq!(server.url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(server.headers.get("Authorization").map(|s| s.as_str()), Some("Bearer token123"));

        // 验证输出包含 type 字段
        let output = ClaudeMcpAdapter::config_to_json_object(&server);
        assert_eq!(output.get("type").and_then(|v| v.as_str()), Some("streamable-http"));
    }

    #[test]
    fn preserves_unknown_fields() {
        let json_str = r#"{
            "theme": "dark",
            "mcpServers": {
                "test": { "command": "npx" }
            },
            "otherConfig": { "key": "value" }
        }"#;
        let mut root: JsonValue = serde_json::from_str(json_str).unwrap();

        // 添加一个新 server
        let mcp_servers = root
            .as_object_mut()
            .unwrap()
            .get_mut("mcpServers")
            .unwrap()
            .as_object_mut()
            .unwrap();
        mcp_servers.insert(
            "new-server".to_string(),
            ClaudeMcpAdapter::config_to_json_object(&McpServerConfig {
                name: "new-server".to_string(),
                transport: McpTransport::Stdio,
                command: Some("node".to_string()),
                args: vec![],
                env: Default::default(),
                url: None,
                headers: Default::default(),
                disabled: false,
                timeout_sec: None,
            }),
        );
        let server_count = mcp_servers.len();

        // 验证其他字段保留（需要先释放 mutable borrow）
        assert_eq!(root.get("theme").and_then(|v| v.as_str()), Some("dark"));
        assert!(root.get("otherConfig").is_some());
        assert_eq!(server_count, 2);
    }
}
