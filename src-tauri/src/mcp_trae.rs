use crate::{
    error::{AppError, AppResult},
    mcp_adapter::{self, McpAdapter},
    models::{AgentMcpServer, AgentProfile, McpServerConfig, McpTransport},
};
use serde_json::Value as JsonValue;
use std::path::{Path, PathBuf};

/// Trae MCP 适配器，读写 `~/.trae/mcp.json`
/// Trae 使用标准 mcpServers JSON 格式，与 Claude 格式一致
const AGENT_LABEL: &str = "Trae";
const SECTION_KEY: &str = "mcpServers";

pub struct TraeMcpAdapter;

impl TraeMcpAdapter {
    pub fn new() -> Self {
        Self
    }

    /// 获取默认配置文件路径
    fn default_config_path() -> Option<PathBuf> {
        // Trae 把 MCP 配置存在 %APPDATA%\Trae\User\mcp.json
        dirs::config_dir().map(|config| config.join("Trae").join("User").join("mcp.json"))
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
            if obj
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
        let timeout_sec = obj.get("timeout").and_then(|v| v.as_u64());

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
                    // 与 Claude 格式一致：HTTP 类型需声明 type 字段
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

impl McpAdapter for TraeMcpAdapter {
    fn scan(&self, profile: &AgentProfile) -> AppResult<Vec<AgentMcpServer>> {
        let path = self.config_path(profile)?;
        let root = mcp_adapter::read_json_file(&path)?;
        let mut servers = Vec::new();

        if let Some(mcp_obj) = root.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, value) in mcp_obj {
                if let Some(obj) = value.as_object() {
                    let server_config = Self::parse_mcp_server(name, obj);
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
                "MCP server '{}' 已存在于 Trae 配置中",
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
                "MCP server '{}' 不存在于 Trae 配置中",
                original_name
            )));
        }

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
                "MCP server '{}' 不存在于 Trae 配置中",
                name
            )));
        }

        mcp_adapter::write_json_file(&path, &root)
    }

    fn toggle(&self, _profile: &AgentProfile, _name: &str, _disabled: bool) -> AppResult<()> {
        Err(AppError::Message(
            "Trae 不支持禁用单个 MCP server".to_string(),
        ))
    }

    fn backup(&self, profile: &AgentProfile, backup_root: &Path) -> AppResult<PathBuf> {
        let path = self.config_path(profile)?;
        mcp_adapter::backup_config_file(&path, backup_root, "trae-mcp", "json", AGENT_LABEL)
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
    fn parses_stdio_mcp() {
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
        let obj = root.get("mcpServers").and_then(|v| v.as_object()).unwrap();
        let server = TraeMcpAdapter::parse_mcp_server(
            "example",
            obj.get("example").unwrap().as_object().unwrap(),
        );

        assert_eq!(server.name, "example");
        assert_eq!(server.transport, McpTransport::Stdio);
        assert_eq!(server.command.as_deref(), Some("npx"));
        assert_eq!(server.args, vec!["-y", "some-mcp-server"]);
        assert_eq!(
            server.env.get("API_KEY").map(|s| s.as_str()),
            Some("test-key")
        );
    }

    #[test]
    fn parses_sse_mcp() {
        let json_str = r#"{
            "mcpServers": {
                "stream": {
                    "url": "https://example.com/sse",
                    "transport": "sse",
                    "headers": { "Authorization": "Bearer token" }
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root.get("mcpServers").and_then(|v| v.as_object()).unwrap();
        let server = TraeMcpAdapter::parse_mcp_server(
            "stream",
            obj.get("stream").unwrap().as_object().unwrap(),
        );

        assert_eq!(server.transport, McpTransport::Sse);
        assert_eq!(server.url.as_deref(), Some("https://example.com/sse"));
        assert_eq!(
            server.headers.get("Authorization").map(|s| s.as_str()),
            Some("Bearer token")
        );
    }

    #[test]
    fn parses_http_mcp() {
        let json_str = r#"{
            "mcpServers": {
                "web": {
                    "url": "https://example.com/mcp",
                    "transport": "http"
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root.get("mcpServers").and_then(|v| v.as_object()).unwrap();
        let server = TraeMcpAdapter::parse_mcp_server(
            "web",
            obj.get("web").unwrap().as_object().unwrap(),
        );

        assert_eq!(server.transport, McpTransport::Http);
    }
}
