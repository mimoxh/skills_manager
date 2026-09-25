use crate::{
    error::{AppError, AppResult},
    models::{ConflictPolicy, McpServerConfig, McpTransport, RemoteInstallOptions, RemoteMcpInstallOptions},
    service::AppService,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{self, BufRead, Write},
};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

pub fn run_stdio(service: AppService) -> AppResult<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(trimmed) {
            if let Some(resp) = handle_request(&service, req) {
                let serialized = serde_json::to_string(&resp)?;
                writer.write_all(serialized.as_bytes())?;
                writer.write_all(b"\n")?;
                writer.flush()?;
            }
        }
        line.clear();
    }

    Ok(())
}

fn handle_request(service: &AppService, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = match req.id {
        Some(id) => id,
        None => {
            // Notification: no response
            return None;
        }
    };

    let result = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "skills-manager",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": get_tool_definitions()
        })),
        "tools/call" => {
            let params = req.params.unwrap_or_else(|| json!({}));
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            handle_tool_call(service, tool_name, &arguments)
        }
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("Method not found: {}", req.method),
            data: None,
        }),
    };

    match result {
        Ok(val) => Some(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(val),
            error: None,
        }),
        Err(err) => Some(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(err),
        }),
    }
}

fn get_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "list_agents",
            "description": "列出当前受 Skills Manager 管理的所有 Agent 与目录路径，包含 Universal 中枢及各 Agent 是否支持 MCP",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "list_skills",
            "description": "列出所有已安装的技能及其在各 Agent 中的分布与版本",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "inspect_source",
            "description": "探测本地目录或 GitHub 仓库中的 Skills 与 MCP 服务，并列出系统可用的目标安装目录",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "GitHub 仓库地址（如 https://github.com/owner/repo 或 owner/repo）或本地路径"
                    }
                },
                "required": ["source"]
            }
        }),
        json!({
            "name": "install_skill",
            "description": "安装技能到指定 Agent 目录或 Universal 中枢。注意：如果未指定 target_agent_ids，工具会自动探测来源并返回候选目录与确认指引，请在向用户提问确认后再携带 target_agent_ids 参数调用！",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "GitHub 仓库地址（支持子目录如 /tree/main/skills/foo）或本地绝对路径"
                    },
                    "target_agent_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "目标 Agent ID 数组；传空数组表示仅保存到中枢，未提供则返回确认指引"
                    },
                    "to_hub": {
                        "type": "boolean",
                        "description": "兼容旧调用；仅允许 true，Skill 始终保存到中枢"
                    },
                    "conflict_policy": {
                        "type": "string",
                        "enum": ["backupOverwrite", "skip", "rename"],
                        "description": "冲突解决策略，默认为 backupOverwrite"
                    },
                    "skill_name": {
                        "type": "string",
                        "description": "在多技能仓库中指定要安装的特定技能名称"
                    }
                },
                "required": ["source"]
            }
        }),
        json!({
            "name": "uninstall_skill",
            "description": "从指定的 Agent 或所有 Agent 中卸载指定技能",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "skill_id": {
                        "type": "string",
                        "description": "技能名称或 ID"
                    },
                    "agent_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "目标 Agent ID 数组，留空表示从所有已安装该技能的 Agent 中卸载"
                    }
                },
                "required": ["skill_id"]
            }
        }),
        json!({
            "name": "list_mcp_servers",
            "description": "列出当前系统中各 Agent 配置的 MCP 服务器列表",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "install_mcp_server",
            "description": "配置或安装 MCP 服务到支持 MCP 的 Agent。如果未指定 agent_ids，将返回可用 MCP Agent 列表并提示向用户确认。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "MCP 服务名称"
                    },
                    "source": {
                        "type": "string",
                        "description": "GitHub 仓库地址（可选，提供则自动探测并填充启动命令）"
                    },
                    "command": {
                        "type": "string",
                        "description": "启动命令（如 node, npx, uv, python 等）"
                    },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "启动参数数组"
                    },
                    "env": {
                        "type": "object",
                        "description": "环境变量键值对"
                    },
                    "agent_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "要配置到的目标 Agent ID 数组"
                    },
                    "conflict_policy": {
                        "type": "string",
                        "enum": ["backupOverwrite", "skip", "rename"]
                    }
                }
            }
        }),
        json!({
            "name": "remove_mcp_server",
            "description": "从指定 Agent 中移除 MCP 服务器",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "MCP 服务名称"
                    },
                    "agent_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "目标 Agent ID 数组，留空表示从所有配置了该服务的 Agent 中移除"
                    }
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "sync_skills",
            "description": "立即触发 Skills 多设备云同步与 Universal 中枢状态刷新",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]
}

fn handle_tool_call(service: &AppService, name: &str, args: &Value) -> Result<Value, JsonRpcError> {
    match name {
        "list_agents" => {
            let agents = service.list_agents().map_err(to_jsonrpc_err)?;
            let list: Vec<Value> = agents
                .into_iter()
                .map(|a| {
                    let is_hub = a.agent_type == crate::models::AgentType::Universal
                        || crate::util::is_universal_skills_path(&a.skills_path);
                    let supports_mcp = matches!(
                        a.agent_type,
                        crate::models::AgentType::Codex
                            | crate::models::AgentType::ClaudeCode
                            | crate::models::AgentType::OpenCode
                            | crate::models::AgentType::Trae
                    );
                    json!({
                        "id": a.id,
                        "name": a.name,
                        "type": a.agent_type.as_str(),
                        "skillsPath": a.skills_path,
                        "isHub": is_hub,
                        "supportsUniversal": a.supports_universal,
                        "supportsMcp": supports_mcp,
                    })
                })
                .collect();
            Ok(tool_success(serde_json::to_string_pretty(&list).unwrap_or_default()))
        }
        "list_skills" => {
            let skills = service.scan_agent_skills().map_err(to_jsonrpc_err)?;
            let list: Vec<Value> = skills
                .into_iter()
                .map(|s| {
                    json!({
                        "title": s.title,
                        "description": s.best_copy.description,
                        "version": s.best_copy.version,
                        "installedAgents": s.installed_agent_ids,
                        "isUniversal": s.is_universal,
                    })
                })
                .collect();
            Ok(tool_success(serde_json::to_string_pretty(&list).unwrap_or_default()))
        }
        "inspect_source" => {
            let source = args
                .get("source")
                .and_then(|v| v.as_str())
                .ok_or_else(|| invalid_params("缺失必填参数 'source'"))?;
            let inspection = service
                .inspect_remote_source(source)
                .map_err(to_jsonrpc_err)?;
            Ok(tool_success(
                serde_json::to_string_pretty(&inspection).unwrap_or_default(),
            ))
        }
        "install_skill" => {
            let source = args
                .get("source")
                .and_then(|v| v.as_str())
                .ok_or_else(|| invalid_params("缺失必填参数 'source'"))?;

            let target_agent_ids: Vec<String> = args
                .get("target_agent_ids")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(ToString::to_string))
                        .collect()
                })
                .unwrap_or_default();

            // 未提供参数才进入确认；显式 [] 表示只保存到中枢。
            if args.get("target_agent_ids").is_none() {
                let inspection = service
                    .inspect_remote_source(source)
                    .map_err(to_jsonrpc_err)?;

                let mut agent_list_text = String::new();
                for (idx, agent) in inspection.available_agents.iter().enumerate() {
                    let tag = if agent.is_hub {
                        " [推荐: Universal 中枢，多端同步]"
                    } else {
                        ""
                    };
                    agent_list_text.push_str(&format!(
                        "{}. {} (ID: \"{}\", 目录: \"{}\"){}\n",
                        idx + 1,
                        agent.name,
                        agent.id,
                        agent.skills_dir,
                        tag
                    ));
                }

                let skill_names: Vec<String> = inspection
                    .skills
                    .iter()
                    .map(|s| format!("{} (v{})", s.title, s.version.as_deref().unwrap_or("1.0.0")))
                    .collect();

                let confirmation_text = format!(
                    "【需要向用户确认安装目录】\n\
                    已识别到仓库中的技能：{}\n\n\
                    本机当前可安装的目标目录如下：\n{}\n\
                    请向用户提问：“要链接到哪些 Agent？也可以只保存到中枢。”\n\
                    在用户确认后，请再次调用 `install_skill`，并将用户选择的 ID 填入 `target_agent_ids` 参数（如 target_agent_ids: [\"{}\"]）。",
                    if skill_names.is_empty() { "待解析技能".to_string() } else { skill_names.join(", ") },
                    agent_list_text,
                    inspection.recommended_agent_ids.first().map(String::as_str).unwrap_or("universal")
                );

                return Ok(tool_confirmation(confirmation_text, json!({
                    "status": "needs_confirmation",
                    "source": source,
                    "skills": inspection.skills,
                    "availableAgents": inspection.available_agents,
                    "recommendedAgentIds": inspection.recommended_agent_ids
                })));
            }

            let to_hub = args.get("to_hub").and_then(|v| v.as_bool()).unwrap_or(true);
            let conflict_policy = parse_conflict_policy(args.get("conflict_policy"));
            let skill_name = args.get("skill_name").and_then(|v| v.as_str());

            let selected_skills = skill_name
                .map(|s| vec![s.to_string()])
                .unwrap_or_default();

            let res = service
                .install_remote_source(RemoteInstallOptions {
                    url: source.to_string(),
                    target_agent_ids,
                    conflict_policy,
                    to_hub: Some(to_hub),
                    selected_skills,
                })
                .map_err(to_jsonrpc_err)?;

            Ok(tool_success(format!(
                "安装完成！{}\n详细结果: {}",
                res.message,
                serde_json::to_string_pretty(&res.results).unwrap_or_default()
            )))
        }
        "uninstall_skill" => {
            let skill_id = args
                .get("skill_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| invalid_params("缺失必填参数 'skill_id'"))?;

            let agent_ids: Option<Vec<String>> = args
                .get("agent_ids")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(ToString::to_string))
                        .collect()
                });

            if let Some(ids) = agent_ids {
                service
                    .uninstall_skill_from_agents(skill_id, &ids)
                    .map_err(to_jsonrpc_err)?;
                Ok(tool_success(format!(
                    "已从指定的 {} 个 Agent 中卸载技能 '{}'",
                    ids.len(),
                    skill_id
                )))
            } else {
                let skills = service.scan_agent_skills().map_err(to_jsonrpc_err)?;
                let matching = skills.into_iter().find(|s| s.title == skill_id);
                if let Some(s) = matching {
                    service
                        .uninstall_skill_from_agents(skill_id, &s.installed_agent_ids)
                        .map_err(to_jsonrpc_err)?;
                    Ok(tool_success(format!(
                        "已从所有 {} 个 Agent 中卸载技能 '{}'",
                        s.installed_agent_ids.len(),
                        skill_id
                    )))
                } else {
                    Ok(tool_success(format!("未找到技能 '{}'，可能已被卸载", skill_id)))
                }
            }
        }
        "list_mcp_servers" => {
            let agents = service.list_agents().map_err(to_jsonrpc_err)?;
            let (servers, warnings) = service.mcp().scan_mcp_servers(&agents).map_err(to_jsonrpc_err)?;
            let text = format!(
                "MCP 服务列表 (共 {} 项):\n{}\n{}",
                servers.len(),
                serde_json::to_string_pretty(&servers).unwrap_or_default(),
                if warnings.is_empty() {
                    String::new()
                } else {
                    format!("警告信息:\n{}", warnings.join("\n"))
                }
            );
            Ok(tool_success(text))
        }
        "install_mcp_server" => {
            let mut name = args.get("name").and_then(|v| v.as_str()).map(ToString::to_string);
            let mut command = args.get("command").and_then(|v| v.as_str()).map(ToString::to_string);
            let mut cmd_args: Vec<String> = args
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(ToString::to_string)).collect())
                .unwrap_or_default();
            let env: HashMap<String, String> = args
                .get("env")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();

            // 如果提供了 source URL 且参数未齐全，尝试探测填充
            if let Some(source) = args.get("source").and_then(|v| v.as_str()) {
                if let Ok(inspection) = service.inspect_remote_source(source) {
                    if let Some(mcp) = inspection.mcp_servers.first() {
                        if name.is_none() {
                            name = Some(mcp.name.clone());
                        }
                        if command.is_none() {
                            command = mcp.command.clone();
                        }
                        if cmd_args.is_empty() {
                            cmd_args = mcp.args.clone();
                        }
                    }
                }
            }

            let target_agent_ids: Vec<String> = args
                .get("agent_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(ToString::to_string)).collect())
                .unwrap_or_default();

            // 若未指定目标 Agent，返回可用 MCP Agent 列表并提示向用户确认
            if target_agent_ids.is_empty() {
                let agents = service.list_agents().map_err(to_jsonrpc_err)?;
                let mcp_agents: Vec<_> = agents
                    .into_iter()
                    .filter(|a| {
                        matches!(
                            a.agent_type,
                            crate::models::AgentType::Codex
                                | crate::models::AgentType::ClaudeCode
                                | crate::models::AgentType::OpenCode
                                | crate::models::AgentType::Trae
                        )
                    })
                    .collect();

                let mut options_text = String::new();
                for (idx, a) in mcp_agents.iter().enumerate() {
                    options_text.push_str(&format!("{}. {} (ID: \"{}\")\n", idx + 1, a.name, a.id));
                }

                let prompt_text = format!(
                    "【需要向用户确认 MCP 目标 Agent】\n\
                    待配置服务: {}\n\
                    支持 MCP 的本机 Agent 列表：\n{}\n\
                    请向用户询问：“请问要将该 MCP 服务配置到哪个/哪些 Agent？”\n\
                    获得用户确认后，带上 `agent_ids` 参数再次调用 `install_mcp_server`。",
                    name.as_deref().unwrap_or("未命名服务"),
                    options_text
                );

                return Ok(tool_confirmation(prompt_text, json!({
                    "status": "needs_confirmation",
                    "mcpName": name,
                    "availableMcpAgents": mcp_agents.into_iter().map(|a| json!({ "id": a.id, "name": a.name })).collect::<Vec<_>>()
                })));
            }

            let server_name = name.ok_or_else(|| invalid_params("缺失 MCP 服务名称 'name'"))?;
            let config = McpServerConfig {
                name: server_name,
                transport: McpTransport::Stdio,
                command,
                args: cmd_args,
                env,
                url: None,
                headers: HashMap::new(),
                disabled: false,
                timeout_sec: None,
            };

            let conflict_policy = parse_conflict_policy(args.get("conflict_policy"));
            let results = service
                .install_remote_mcp(RemoteMcpInstallOptions {
                    config,
                    target_agent_ids,
                    conflict_policy,
                })
                .map_err(to_jsonrpc_err)?;

            Ok(tool_success(format!(
                "MCP 服务配置完成！结果:\n{}",
                serde_json::to_string_pretty(&results).unwrap_or_default()
            )))
        }
        "remove_mcp_server" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| invalid_params("缺失必填参数 'name'"))?;

            let target_agent_ids: Option<Vec<String>> = args
                .get("agent_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(ToString::to_string)).collect());

            let agents = service.list_agents().map_err(to_jsonrpc_err)?;
            let results = if let Some(ids) = target_agent_ids {
                service.mcp().remove_mcp_server_from_agents(&agents, name, &ids)
            } else {
                let (servers, _) = service.mcp().scan_mcp_servers(&agents).map_err(to_jsonrpc_err)?;
                let matching = servers.into_iter().find(|s| s.name == name);
                if let Some(server) = matching {
                    service.mcp().remove_mcp_server_from_agents(&agents, name, &server.agent_ids)
                } else {
                    return Ok(tool_success(format!("未找到名为 '{}' 的 MCP 服务", name)));
                }
            }
            .map_err(to_jsonrpc_err)?;

            Ok(tool_success(format!(
                "已移除 MCP 服务 '{}'。结果:\n{}",
                name,
                serde_json::to_string_pretty(&results).unwrap_or_default()
            )))
        }
        "sync_skills" => {
            let status = service.sync_now().map_err(to_jsonrpc_err)?;
            Ok(tool_success(format!(
                "多端同步触发完成。状态:\n{}",
                serde_json::to_string_pretty(&status).unwrap_or_default()
            )))
        }
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("未知工具: {}", name),
            data: None,
        }),
    }
}

fn tool_success(text: String) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "isError": false
    })
}

fn tool_confirmation(prompt: String, meta: Value) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": prompt
            }
        ],
        "metadata": meta,
        "isError": false
    })
}

fn parse_conflict_policy(val: Option<&Value>) -> ConflictPolicy {
    match val.and_then(|v| v.as_str()) {
        Some("skip") => ConflictPolicy::Skip,
        Some("rename") => ConflictPolicy::Rename,
        _ => ConflictPolicy::BackupOverwrite,
    }
}

fn to_jsonrpc_err(err: AppError) -> JsonRpcError {
    JsonRpcError {
        code: -32000,
        message: err.to_string(),
        data: None,
    }
}

fn invalid_params(msg: &str) -> JsonRpcError {
    JsonRpcError {
        code: -32602,
        message: msg.to_string(),
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_initialize_and_tools_list() {
        let service = AppService::in_memory().unwrap();

        // 1. initialize
        let init_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: None,
        };
        let resp = handle_request(&service, init_req).unwrap();
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap().get("serverInfo").is_some());

        // 2. tools/list
        let tools_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = handle_request(&service, tools_req).unwrap();
        let tools = resp.result.unwrap().get("tools").cloned().unwrap();
        let tools_arr = tools.as_array().unwrap();
        assert!(tools_arr.iter().any(|t| t["name"] == "install_skill"));
        assert!(tools_arr.iter().any(|t| t["name"] == "list_agents"));
    }

    #[test]
    fn install_skill_without_targets_returns_needs_confirmation() {
        let service = AppService::in_memory().unwrap();

        let temp = tempfile::tempdir().unwrap();
        let skill_dir = temp.path().join("local-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "---\nname: Test\n---\n").unwrap();

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(4)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "install_skill",
                "arguments": {
                    "source": temp.path().to_string_lossy().to_string()
                }
            })),
        };
        let resp = handle_request(&service, req).unwrap();
        let res = resp.result.unwrap();
        assert_eq!(res["isError"], false);
        let meta = &res["metadata"];
        assert_eq!(meta["status"], "needs_confirmation");
        assert!(meta["availableAgents"].is_array());
    }
}
