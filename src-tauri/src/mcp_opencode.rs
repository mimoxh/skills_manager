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

/// OpenCode MCP 适配器，读写 `~/.opencode.json`
/// OpenCode 使用 "mcp" 作为顶层 key，"remote" 作为远程传输类型，支持 "enabled" 字段
pub struct OpenCodeMcpAdapter;

/// OpenCode MCP 配置的顶层 key
const MCP_KEY: &str = "mcp";

impl OpenCodeMcpAdapter {
    pub fn new() -> Self {
        Self
    }

    fn opencode_json_path() -> Option<PathBuf> {
        let home = dirs::home_dir()?;

        let home_config = home.join(".opencode.json");
        if home_config.exists() {
            return Some(home_config);
        }

        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            let xdg_path = PathBuf::from(xdg).join("opencode").join(".opencode.json");
            if xdg_path.exists() {
                return Some(xdg_path);
            }
        }

        let dot_config = home.join(".config").join("opencode").join(".opencode.json");
        if dot_config.exists() {
            return Some(dot_config);
        }

        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let local_path = PathBuf::from(local_app_data)
                .join("opencode")
                .join(".opencode.json");
            if local_path.exists() {
                return Some(local_path);
            }
        }

        Some(home_config)
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

    /// 将 env 字符串数组 ["KEY=val", ...] 转换为 HashMap
    fn parse_env_array(arr: &[JsonValue]) -> std::collections::HashMap<String, String> {
        let mut env = std::collections::HashMap::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                if let Some((key, value)) = s.split_once('=') {
                    env.insert(key.to_string(), value.to_string());
                }
            }
        }
        env
    }

    fn parse_mcp_server(name: &str, obj: &serde_json::Map<String, JsonValue>) -> McpServerConfig {
        let type_str = obj
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("local")
            .to_lowercase();

        let url = obj
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // OpenCode: "remote" = SSE/HTTP, "local" (或缺失) = stdio
        let transport = match type_str.as_str() {
            "remote" | "sse" | "http" => McpTransport::Sse,
            _ => {
                if url.is_some() {
                    McpTransport::Sse
                } else {
                    McpTransport::Stdio
                }
            }
        };

        // OpenCode 的 command 是合并数组 ["npx", "-y", "foo"]
        let (command, args) = match obj.get("command") {
            Some(JsonValue::Array(arr)) if !arr.is_empty() => {
                let cmd = arr[0].as_str().map(|s| s.to_string());
                let args: Vec<String> = arr[1..]
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                (cmd, args)
            }
            Some(JsonValue::String(s)) => {
                // 兼容旧格式：command 是字符串，args 是单独数组
                let args = obj
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                (Some(s.to_string()), args)
            }
            _ => (None, vec![]),
        };

        // OpenCode 用 "environment" 作为 env key（也兼容 "env"）
        let env = obj
            .get("environment")
            .or_else(|| obj.get("env"))
            .and_then(|v| {
                // 兼容两种格式：对象 { "KEY": "val" } 或数组 ["KEY=val"]
                match v {
                    JsonValue::Object(o) => Some(
                        o.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect(),
                    ),
                    JsonValue::Array(arr) => Some(Self::parse_env_array(arr)),
                    _ => None,
                }
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

        let enabled = obj
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let timeout_sec = obj.get("timeout").and_then(|v| v.as_u64());

        McpServerConfig {
            name: name.to_string(),
            transport,
            command,
            args,
            env,
            url,
            headers,
            disabled: !enabled,
            timeout_sec,
        }
    }

    fn config_to_json_object(config: &McpServerConfig) -> JsonValue {
        let mut obj = serde_json::Map::new();

        // OpenCode: "local" = stdio, "remote" = SSE/HTTP
        let type_str = match config.transport {
            McpTransport::Stdio => "local",
            McpTransport::Http | McpTransport::Sse => "remote",
        };
        obj.insert(
            "type".to_string(),
            JsonValue::String(type_str.to_string()),
        );

        // enabled 字段
        obj.insert(
            "enabled".to_string(),
            JsonValue::Bool(!config.disabled),
        );

        match config.transport {
            McpTransport::Stdio => {
                // OpenCode 的 command 是合并数组 ["npx", "-y", "foo"]
                let mut cmd_arr: Vec<JsonValue> = Vec::new();
                if let Some(ref cmd) = config.command {
                    cmd_arr.push(JsonValue::String(cmd.clone()));
                }
                for arg in &config.args {
                    cmd_arr.push(JsonValue::String(arg.clone()));
                }
                if !cmd_arr.is_empty() {
                    obj.insert("command".to_string(), JsonValue::Array(cmd_arr));
                }
                // environment 用对象格式 { "KEY": "val" }
                if !config.env.is_empty() {
                    let env: serde_json::Map<String, JsonValue> = config
                        .env
                        .iter()
                        .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
                        .collect();
                    obj.insert("environment".to_string(), JsonValue::Object(env));
                }
            }
            McpTransport::Http | McpTransport::Sse => {
                if let Some(ref url) = config.url {
                    obj.insert("url".to_string(), JsonValue::String(url.clone()));
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

impl McpAdapter for OpenCodeMcpAdapter {
    fn scan(&self, profile: &AgentProfile) -> AppResult<Vec<AgentMcpServer>> {
        let path = self.config_path(profile)?;
        let root = Self::read_json(&path)?;
        let mut servers = Vec::new();

        // 同时检查 "mcp" 和 "mcpServers" 两个 key（兼容性）
        let mcp_obj = root
            .get(MCP_KEY)
            .or_else(|| root.get("mcpServers"))
            .and_then(|v| v.as_object());

        if let Some(mcp_obj) = mcp_obj {
            for (name, value) in mcp_obj {
                if let Some(obj) = value.as_object() {
                    let server_config = Self::parse_mcp_server(name, obj);
                    // 包装为 { "mcp": { "server_name": { ... } } } 完整结构
                    let mut mcp_wrapper = serde_json::Map::new();
                    mcp_wrapper.insert(name.clone(), value.clone());
                    let mut root_wrapper = serde_json::Map::new();
                    root_wrapper.insert(MCP_KEY.to_string(), JsonValue::Object(mcp_wrapper));
                    let raw = serde_json::to_string_pretty(&root_wrapper).ok();
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

        if root.get(MCP_KEY).is_none() {
            root.as_object_mut()
                .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
                .insert(MCP_KEY.to_string(), JsonValue::Object(serde_json::Map::new()));
        }

        let mcp_servers = root
            .as_object_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .get_mut(MCP_KEY)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| AppError::Message(format!("未找到 {} 配置", MCP_KEY)))?;

        if mcp_servers.contains_key(&config.name) {
            return Err(AppError::Message(format!(
                "MCP server '{}' 已存在于 OpenCode 配置中",
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
            .get_mut(MCP_KEY)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| AppError::Message("未找到 mcp 配置".to_string()))?;

        if !mcp_servers.contains_key(original_name) {
            return Err(AppError::Message(format!(
                "MCP server '{}' 不存在于 OpenCode 配置中",
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
            .get_mut(MCP_KEY)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| AppError::Message("未找到 mcp 配置".to_string()))?;

        if mcp_servers.remove(name).is_none() {
            return Err(AppError::Message(format!(
                "MCP server '{}' 不存在于 OpenCode 配置中",
                name
            )));
        }

        Self::write_json(&path, &root)
    }

    fn toggle(&self, profile: &AgentProfile, name: &str, disabled: bool) -> AppResult<()> {
        let path = self.config_path(profile)?;
        let mut root = Self::read_json(&path)?;

        let mcp_servers = root
            .as_object_mut()
            .ok_or_else(|| AppError::Message("配置格式错误".to_string()))?
            .get_mut(MCP_KEY)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| AppError::Message("未找到 mcp 配置".to_string()))?;

        let server_obj = mcp_servers
            .get_mut(name)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| AppError::Message(format!("MCP server '{}' 不存在", name)))?;

        // OpenCode 使用 "enabled" 字段
        server_obj.insert("enabled".to_string(), JsonValue::Bool(!disabled));

        Self::write_json(&path, &root)
    }

    fn backup(&self, profile: &AgentProfile) -> AppResult<PathBuf> {
        let path = self.config_path(profile)?;
        if !path.exists() {
            return Err(AppError::Message("OpenCode 配置文件不存在".to_string()));
        }
        let backup_name = format!(
            "opencode-config-{}.json",
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
        Self::opencode_json_path()
            .ok_or_else(|| AppError::Message("无法确定 OpenCode 配置路径".to_string()))
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

    /// OpenCode 的 stdio 格式：command 是合并数组，environment 是对象
    #[test]
    fn parses_local_mcp_from_json() {
        let json_str = r#"{
            "mcp": {
                "example": {
                    "type": "local",
                    "command": ["npx", "-y", "some-mcp-server"],
                    "environment": { "API_KEY": "test-key" },
                    "enabled": true
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root.get("mcp").and_then(|v| v.as_object()).unwrap();
        let server = OpenCodeMcpAdapter::parse_mcp_server(
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
        assert!(!server.disabled);
    }

    /// OpenCode 的 remote 格式：url + headers
    #[test]
    fn parses_remote_mcp_from_json() {
        let json_str = r#"{
            "mcp": {
                "stream": {
                    "type": "remote",
                    "enabled": true,
                    "url": "https://example.com/mcp",
                    "headers": {
                        "Authorization": "Bearer token"
                    }
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root.get("mcp").and_then(|v| v.as_object()).unwrap();
        let server = OpenCodeMcpAdapter::parse_mcp_server(
            "stream",
            obj.get("stream").unwrap().as_object().unwrap(),
        );

        assert_eq!(server.transport, McpTransport::Sse);
        assert_eq!(server.url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(
            server.headers.get("Authorization").map(|s| s.as_str()),
            Some("Bearer token")
        );
        assert!(!server.disabled);
    }

    #[test]
    fn parses_disabled_mcp() {
        let json_str = r#"{
            "mcp": {
                "old": {
                    "type": "local",
                    "command": ["npx"],
                    "enabled": false
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root.get("mcp").and_then(|v| v.as_object()).unwrap();
        let server = OpenCodeMcpAdapter::parse_mcp_server(
            "old",
            obj.get("old").unwrap().as_object().unwrap(),
        );

        assert!(server.disabled);
    }

    #[test]
    fn defaults_to_local_when_type_missing() {
        let json_str = r#"{
            "mcp": {
                "legacy": {
                    "command": ["node", "server.js"]
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root.get("mcp").and_then(|v| v.as_object()).unwrap();
        let server = OpenCodeMcpAdapter::parse_mcp_server(
            "legacy",
            obj.get("legacy").unwrap().as_object().unwrap(),
        );

        assert_eq!(server.transport, McpTransport::Stdio);
        assert_eq!(server.command.as_deref(), Some("node"));
        assert_eq!(server.args, vec!["server.js"]);
    }

    #[test]
    fn writes_local_type_and_enabled() {
        let config = McpServerConfig {
            name: "test".to_string(),
            transport: McpTransport::Stdio,
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "some-mcp".to_string()],
            env: {
                let mut e = std::collections::HashMap::new();
                e.insert("KEY".to_string(), "val".to_string());
                e
            },
            url: None,
            headers: Default::default(),
            disabled: false,
            timeout_sec: None,
        };
        let json = OpenCodeMcpAdapter::config_to_json_object(&config);
        let obj = json.as_object().unwrap();
        assert_eq!(obj.get("type").and_then(|v| v.as_str()), Some("local"));
        assert_eq!(obj.get("enabled").and_then(|v| v.as_bool()), Some(true));
        // command 是合并数组
        let cmd = obj.get("command").and_then(|v| v.as_array()).unwrap();
        assert_eq!(cmd[0].as_str(), Some("npx"));
        assert_eq!(cmd[1].as_str(), Some("-y"));
        assert_eq!(cmd[2].as_str(), Some("some-mcp"));
        // environment 是对象
        let env = obj.get("environment").and_then(|v| v.as_object()).unwrap();
        assert_eq!(env.get("KEY").and_then(|v| v.as_str()), Some("val"));
    }

    #[test]
    fn writes_remote_type_and_enabled() {
        let config = McpServerConfig {
            name: "test".to_string(),
            transport: McpTransport::Sse,
            command: None,
            args: vec![],
            env: Default::default(),
            url: Some("https://example.com/mcp".to_string()),
            headers: {
                let mut h = std::collections::HashMap::new();
                h.insert("Authorization".to_string(), "Bearer tok".to_string());
                h
            },
            disabled: false,
            timeout_sec: None,
        };
        let json = OpenCodeMcpAdapter::config_to_json_object(&config);
        let obj = json.as_object().unwrap();
        assert_eq!(obj.get("type").and_then(|v| v.as_str()), Some("remote"));
        assert_eq!(obj.get("enabled").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            obj.get("url").and_then(|v| v.as_str()),
            Some("https://example.com/mcp")
        );
    }

    /// 兼容读取旧格式：command 是字符串，env 是数组
    #[test]
    fn parses_legacy_string_command_and_env_array() {
        let json_str = r#"{
            "mcp": {
                "legacy": {
                    "command": "npx",
                    "args": ["-y", "foo"],
                    "env": ["KEY=val"]
                }
            }
        }"#;
        let root: JsonValue = serde_json::from_str(json_str).unwrap();
        let obj = root.get("mcp").and_then(|v| v.as_object()).unwrap();
        let server = OpenCodeMcpAdapter::parse_mcp_server(
            "legacy",
            obj.get("legacy").unwrap().as_object().unwrap(),
        );

        assert_eq!(server.command.as_deref(), Some("npx"));
        assert_eq!(server.args, vec!["-y", "foo"]);
        assert_eq!(server.env.get("KEY").map(|s| s.as_str()), Some("val"));
    }
}
