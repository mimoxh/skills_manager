use crate::{
    error::{AppError, AppResult},
    mcp_adapter::McpAdapter,
    models::{AgentMcpServer, AgentProfile, McpServerConfig, McpTransport},
};
use serde_json::Value as JsonValue;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Trae MCP 适配器，读写 `~/.trae/mcp.json`
/// Trae 使用标准 mcpServers JSON 格式，与 Claude 格式一致
pub struct TraeMcpAdapter;

impl TraeMcpAdapter {
    pub fn new() -> Self {
        Self
    }

    fn trae_mcp_path() -> Option<PathBuf> {
        // Trae 把 MCP 配置存在 %APPDATA%\Trae\User\mcp.json
        dirs::config_dir().map(|config| config.join("Trae").join("User").join("mcp.json"))
    }

    fn read_json(path: &Path) -> AppResult<JsonValue> {
        if !path.exists() {
            return Ok(JsonValue::Object(Default::default()));
        }
        let text = fs::read_to_string(path)?;
        let value: JsonValue = serde_json::from_str(&text)?;
        Ok(value)
    }

    fn write_json(path: &Path, value: &JsonValue) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(value)?;
        fs::write(path, text)?;
        Ok(())
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

        McpServerConfig {
            name: name.to_string(),
            transport,
            command,
            args,
            env,
            url,
            headers,
            disabled: false,
            timeout_sec: None,
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

        JsonValue::Object(obj)
    }
}

impl McpAdapter for TraeMcpAdapter {
    fn scan(&self, profile: &AgentProfile) -> AppResult<Vec<AgentMcpServer>> {
        let path = self.config_path(profile)?;
        let root = Self::read_json(&path)?;
        let mut servers = Vec::new();

        if let Some(mcp_obj) = root.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, value) in mcp_obj {
                if let Some(obj) = value.as_object() {
                    let server_config = Self::parse_mcp_server(name, obj);
                    let mut wrapper = serde_json::Map::new();
                    wrapper.insert(name.clone(), value.clone());
                    let raw = serde_json::to_string_pretty(&wrapper).ok();
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
        let mut root = Self::read_json(&path)?;

        if root.get("mcpServers").is_none() {
            root.as_object_mut()
                .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
                .insert(
                    "mcpServers".to_string(),
                    JsonValue::Object(serde_json::Map::new()),
                );
        }

        let mcp_servers = root
            .as_object_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .get_mut("mcpServers")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| AppError::Message("未找到 mcpServers 配置".to_string()))?;

        if mcp_servers.contains_key(&config.name) {
            return Err(AppError::Message(format!(
                "MCP server '{}' 已存在于 Trae 配置中",
                config.name
            )));
        }

        mcp_servers.insert(config.name.clone(), Self::config_to_json_object(config));

        Self::write_json(&path, &root)
    }

    fn update(
        &self,
        profile: &AgentProfile,
        original_name: &str,
        config: &McpServerConfig,
    ) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = Self::read_json(&path)?;

        let mcp_servers = root
            .as_object_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .get_mut("mcpServers")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| AppError::Message("未找到 mcpServers 配置".to_string()))?;

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

        Self::write_json(&path, &root)
    }

    fn remove(&self, profile: &AgentProfile, name: &str) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = Self::read_json(&path)?;

        let mcp_servers = root
            .as_object_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .get_mut("mcpServers")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| AppError::Message("未找到 mcpServers 配置".to_string()))?;

        if mcp_servers.remove(name).is_none() {
            return Err(AppError::Message(format!(
                "MCP server '{}' 不存在于 Trae 配置中",
                name
            )));
        }

        Self::write_json(&path, &root)
    }

    fn toggle(&self, _profile: &AgentProfile, _name: &str, _disabled: bool) -> AppResult<()> {
        Err(AppError::Message(
            "Trae 不支持禁用单个 MCP server".to_string(),
        ))
    }

    fn backup(&self, profile: &AgentProfile) -> AppResult<PathBuf> {
        let path = self.config_path(profile)?;
        if !path.exists() {
            return Err(AppError::Message("Trae 配置文件不存在".to_string()));
        }
        let backup_name = format!(
            "trae-mcp-{}.json",
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
        if let Some(config) = &profile.adapter_config {
            if let Some(path) = config.get("mcpConfigPath").and_then(|v| v.as_str()) {
                let trimmed = path.trim();
                if !trimmed.is_empty() {
                    return Ok(PathBuf::from(trimmed));
                }
            }
        }
        Self::trae_mcp_path()
            .ok_or_else(|| AppError::Message("无法确定 Trae 配置路径".to_string()))
    }
}

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
                "api": {
                    "url": "https://example.com/mcp"
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root.get("mcpServers").and_then(|v| v.as_object()).unwrap();
        let server = TraeMcpAdapter::parse_mcp_server(
            "api",
            obj.get("api").unwrap().as_object().unwrap(),
        );

        assert_eq!(server.transport, McpTransport::Http);
        assert_eq!(server.url.as_deref(), Some("https://example.com/mcp"));
    }
}
