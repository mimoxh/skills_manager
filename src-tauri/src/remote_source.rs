use crate::{
    error::{AppError, AppResult},
    manifest::{read_skill, synthesize_manifest_from_skill_md},
    models::{
        DetectedMcpInfo, DetectedSkillInfo, McpTransport, RemoteSourceType,
    },
};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::Duration,
};
use zip::ZipArchive;

const MAX_REMOTE_ZIP_BYTES: u64 = 100 * 1024 * 1024; // 100 MB

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedGitUrl {
    pub raw_url: String,
    pub clone_url: String,
    pub owner: String,
    pub repo: String,
    pub branch: Option<String>,
    pub subpath: Option<String>,
    pub is_github: bool,
}

/// 解析各种形态的 GitHub 与通用 Git URL。
/// 支持：
/// - https://github.com/owner/repo
/// - https://github.com/owner/repo.git
/// - https://github.com/owner/repo/tree/branch/subpath
/// - https://github.com/owner/repo/blob/branch/subpath/SKILL.md
/// - git@github.com:owner/repo.git
/// - owner/repo 或 owner/repo/tree/branch/subpath
pub fn parse_git_url(raw: &str) -> AppResult<ParsedGitUrl> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Message("Git URL 不能为空".to_string()));
    }

    // 格式 1: git@github.com:owner/repo.git
    if let Some(ssh_part) = trimmed.strip_prefix("git@") {
        if let Some((host, path)) = ssh_part.split_once(':') {
            let path = path.trim_end_matches(".git").trim_matches('/');
            let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            if segments.len() >= 2 {
                let owner = segments[0].to_string();
                let repo = segments[1].to_string();
                let clone_url = format!("https://{}/{}/{}.git", host, owner, repo);
                return Ok(ParsedGitUrl {
                    raw_url: trimmed.to_string(),
                    clone_url,
                    owner,
                    repo,
                    branch: None,
                    subpath: None,
                    is_github: host.contains("github.com"),
                });
            }
        }
    }

    let url_str = if !trimmed.contains("://") && !trimmed.starts_with("git@") {
        format!("https://github.com/{}", trimmed.trim_start_matches('/'))
    } else {
        trimmed.to_string()
    };

    // 格式 2: HTTP(S) URL
    let without_scheme = if let Some(idx) = url_str.find("://") {
        &url_str[idx + 3..]
    } else {
        &url_str
    };

    let segments: Vec<&str> = without_scheme
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if segments.len() < 3 {
        return Err(AppError::Message(format!(
            "无法识别的 Git 仓库地址: '{}'，格式应为 owner/repo 或完整 GitHub URL",
            trimmed
        )));
    }

    let host = segments[0];
    let is_github = host.contains("github.com");
    let owner = segments[1].to_string();
    let repo = segments[2].trim_end_matches(".git").to_string();

    let mut branch = None;
    let mut subpath = None;

    // 解析 /tree/{branch}/... 或 /blob/{branch}/...
    if segments.len() >= 5 && (segments[3] == "tree" || segments[3] == "blob") {
        branch = Some(segments[4].to_string());
        if segments.len() > 5 {
            let sub_segments = &segments[5..];
            let joined = sub_segments.join("/");
            // 如果末尾是具体文件如 SKILL.md 或 skill.json，截取其所在目录
            if joined.ends_with(".md") || joined.ends_with(".json") || joined.ends_with(".yaml") || joined.ends_with(".yml") {
                if let Some((dir, _)) = joined.rsplit_once('/') {
                    subpath = Some(dir.to_string());
                }
            } else {
                subpath = Some(joined);
            }
        }
    }

    // 规范化 clone URL
    let clone_url = format!("https://{}/{}/{}.git", host, owner, repo);

    Ok(ParsedGitUrl {
        raw_url: trimmed.to_string(),
        clone_url,
        owner,
        repo,
        branch,
        subpath,
        is_github,
    })
}

pub struct RemoteFetcher {
    cache_root: PathBuf,
}

impl RemoteFetcher {
    pub fn new(cache_root: PathBuf) -> Self {
        Self { cache_root }
    }

    /// 拉取远端源码到本地临时/缓存目录。
    /// 策略：优先使用系统 git clone --depth 1；若失败或无 git，且为 GitHub，回退到 Zip Archive 下载。
    pub fn fetch(&self, parsed: &ParsedGitUrl) -> AppResult<PathBuf> {
        let repo_dir_name = format!("{}_{}", parsed.owner, parsed.repo);
        let target_dir = self.cache_root.join(&repo_dir_name);

        // 如果之前已经拉取过且有效，清理旧目录重新克隆以确保最新
        if target_dir.exists() {
            let _ = fs::remove_dir_all(&target_dir);
        }
        fs::create_dir_all(&target_dir)
            .map_err(|e| AppError::Message(format!("无法创建缓存目录: {}", e)))?;

        // 尝试 1: git clone
        let git_result = self.try_git_clone(parsed, &target_dir);
        if git_result.is_ok() {
            return Ok(target_dir);
        }

        // 尝试 2: GitHub Archive Zip 兜底
        if parsed.is_github {
            let zip_result = self.try_github_zip_download(parsed, &target_dir);
            if let Ok(extracted_dir) = zip_result {
                return Ok(extracted_dir);
            }
        }

        Err(AppError::Message(format!(
            "拉取仓库失败 (git 错误: {:?})。请检查网络连接、Git 安装或仓库地址是否正确。",
            git_result.err()
        )))
    }

    fn try_git_clone(&self, parsed: &ParsedGitUrl, target_dir: &Path) -> Result<(), String> {
        let mut cmd = crate::util::command_no_window("git");
        cmd.arg("clone").arg("--depth").arg("1");
        if let Some(branch) = &parsed.branch {
            cmd.arg("--branch").arg(branch);
        }
        cmd.arg(&parsed.clone_url).arg(target_dir);

        let output = cmd.output().map_err(|e| format!("执行 git 失败: {}", e))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(stderr.to_string())
        }
    }

    fn try_github_zip_download(&self, parsed: &ParsedGitUrl, target_dir: &Path) -> AppResult<PathBuf> {
        let candidate_branches = if let Some(b) = &parsed.branch {
            vec![b.as_str()]
        } else {
            vec!["main", "master"]
        };

        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(45))
            .build();

        let mut downloaded_bytes = None;

        for branch in candidate_branches {
            let zip_url = format!(
                "https://github.com/{}/{}/archive/refs/heads/{}.zip",
                parsed.owner, parsed.repo, branch
            );
            match agent.get(&zip_url).call() {
                Ok(resp) => {
                    let reader = resp.into_reader();
                    let mut bytes = Vec::new();
                    if reader
                        .take(MAX_REMOTE_ZIP_BYTES + 1)
                        .read_to_end(&mut bytes)
                        .is_ok()
                        && bytes.len() as u64 <= MAX_REMOTE_ZIP_BYTES
                    {
                        downloaded_bytes = Some(bytes);
                        break;
                    }
                }
                Err(_) => continue,
            }
        }

        let bytes = downloaded_bytes.ok_or_else(|| {
            AppError::Message("通过 GitHub Archive Zip 下载仓库失败".to_string())
        })?;

        // 解压 zip 包
        let cursor = Cursor::new(bytes);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| AppError::Message(format!("解析 Zip 失败: {}", e)))?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| AppError::Message(format!("解压条目失败: {}", e)))?;
            let enclosed = match file.enclosed_name() {
                Some(name) => name.to_owned(),
                None => continue,
            };
            let out_path = target_dir.join(enclosed);
            if file.is_dir() {
                fs::create_dir_all(&out_path)
                    .map_err(|e| AppError::Message(format!("创建目录失败: {}", e)))?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| AppError::Message(format!("创建父目录失败: {}", e)))?;
                }
                let mut out_file = File::create(&out_path)
                    .map_err(|e| AppError::Message(format!("写入文件失败: {}", e)))?;
                std::io::copy(&mut file, &mut out_file)
                    .map_err(|e| AppError::Message(format!("写入内容失败: {}", e)))?;
            }
        }

        // GitHub zip 包通常第一层为 {repo}-{branch} 目录，如果存在且唯一，返回该子目录
        if let Ok(entries) = fs::read_dir(target_dir) {
            let valid_dirs: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            if valid_dirs.len() == 1 {
                return Ok(valid_dirs[0].clone());
            }
        }

        Ok(target_dir.to_path_buf())
    }
}

/// 探测本地目录（通常为拉取下来的远端仓库）中的 Skills 与 MCP 配置
pub fn inspect_directory(root: &Path, subpath: Option<&str>) -> AppResult<(Vec<DetectedSkillInfo>, Vec<DetectedMcpInfo>, RemoteSourceType)> {
    let target_root = if let Some(sub) = subpath {
        let joined = root.join(sub);
        if joined.exists() {
            joined
        } else {
            root.to_path_buf()
        }
    } else {
        root.to_path_buf()
    };

    let mut detected_skills = Vec::new();
    let mut detected_mcp = Vec::new();

    // 1. 检查当前目标目录是否直接是一个 Skill
    if let Some(skill_info) = check_dir_is_skill(&target_root, "") {
        detected_skills.push(skill_info);
    } else {
        // 向下扫描遍历找 skills (深度限制 ≤ 3)
        scan_for_skills_recursive(&target_root, &target_root, 0, &mut detected_skills);
    }

    // 2. 检查是否存在 MCP Server 标识
    if let Some(mcp_info) = detect_mcp_in_dir(&target_root) {
        detected_mcp.push(mcp_info);
    }

    let detected_type = match (!detected_skills.is_empty(), !detected_mcp.is_empty()) {
        (true, true) => RemoteSourceType::Both,
        (true, false) => {
            if detected_skills.len() > 1 {
                RemoteSourceType::MultiSkill
            } else {
                RemoteSourceType::SingleSkill
            }
        }
        (false, true) => RemoteSourceType::McpServer,
        (false, false) => RemoteSourceType::Unknown,
    };

    Ok((detected_skills, detected_mcp, detected_type))
}

fn check_dir_is_skill(dir: &Path, rel_path: &str) -> Option<DetectedSkillInfo> {
    // 优先匹配 skill.json / skill.yaml / skill.yml
    for manifest_name in &["skill.json", "skill.yaml", "skill.yml"] {
        let p = dir.join(manifest_name);
        if p.exists() {
            if let Ok(skill) = read_skill(&p) {
                return Some(DetectedSkillInfo {
                    name: skill.manifest.id,
                    title: skill.manifest.name,
                    description: skill.manifest.description.unwrap_or_default(),
                    version: Some(skill.manifest.version),
                    relative_path: rel_path.to_string(),
                });
            }
        }
    }

    // 回退 SKILL.md
    let skill_md = dir.join("SKILL.md");
    if skill_md.exists() {
        if let Ok(skill) = synthesize_manifest_from_skill_md(&skill_md) {
            return Some(DetectedSkillInfo {
                name: skill.manifest.id,
                title: skill.manifest.name,
                description: skill.manifest.description.unwrap_or_default(),
                version: Some(skill.manifest.version),
                relative_path: rel_path.to_string(),
            });
        }
    }

    None
}

fn scan_for_skills_recursive(
    current: &Path,
    root: &Path,
    depth: usize,
    results: &mut Vec<DetectedSkillInfo>,
) {
    if depth > 3 {
        return;
    }

    let entries = match fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" || name == "dist" {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        if let Some(info) = check_dir_is_skill(&path, &rel) {
            results.push(info);
        } else {
            scan_for_skills_recursive(&path, root, depth + 1, results);
        }
    }
}

fn detect_mcp_in_dir(dir: &Path) -> Option<DetectedMcpInfo> {
    let dir_name = dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("mcp-server")
        .to_string();

    // 1. 检查 mcp.json
    let mcp_json = dir.join("mcp.json");
    if mcp_json.exists() {
        if let Ok(content) = fs::read_to_string(&mcp_json) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let name = val
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&dir_name)
                    .to_string();
                let command = val
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                let args = val
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                return Some(DetectedMcpInfo {
                    name,
                    description: "从 mcp.json 检测到的 MCP 服务".to_string(),
                    transport: McpTransport::Stdio,
                    command,
                    args,
                    env: HashMap::new(),
                });
            }
        }
    }

    // 2. 检查 package.json
    let package_json = dir.join("package.json");
    if package_json.exists() {
        if let Ok(content) = fs::read_to_string(&package_json) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let pkg_name = val
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&dir_name)
                    .to_string();
                let desc = val
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("基于 Node.js 的 MCP 服务")
                    .to_string();

                let deps = val.get("dependencies").and_then(|v| v.as_object());
                let dev_deps = val.get("devDependencies").and_then(|v| v.as_object());
                let has_mcp_sdk = deps.map_or(false, |d| d.contains_key("@modelcontextprotocol/sdk"))
                    || dev_deps.map_or(false, |d| d.contains_key("@modelcontextprotocol/sdk"));

                let has_bin = val.get("bin").is_some();

                if has_mcp_sdk || has_bin {
                    let (cmd, args) = if dir.join("dist/index.js").exists() {
                        ("node".to_string(), vec!["dist/index.js".to_string()])
                    } else if dir.join("index.js").exists() {
                        ("node".to_string(), vec!["index.js".to_string()])
                    } else {
                        ("npx".to_string(), vec!["-y".to_string(), pkg_name.clone()])
                    };

                    return Some(DetectedMcpInfo {
                        name: pkg_name,
                        description: desc,
                        transport: McpTransport::Stdio,
                        command: Some(cmd),
                        args,
                        env: HashMap::new(),
                    });
                }
            }
        }
    }

    // 3. 检查 pyproject.toml 或 requirements.txt
    let pyproject = dir.join("pyproject.toml");
    let requirements = dir.join("requirements.txt");
    if pyproject.exists() || requirements.exists() {
        let is_mcp = if pyproject.exists() {
            fs::read_to_string(&pyproject)
                .map(|s| s.contains("mcp") || s.contains("fastmcp"))
                .unwrap_or(false)
        } else {
            fs::read_to_string(&requirements)
                .map(|s| s.contains("mcp") || s.contains("fastmcp"))
                .unwrap_or(false)
        };

        if is_mcp {
            let server_script = if dir.join("server.py").exists() {
                "server.py"
            } else if dir.join("main.py").exists() {
                "main.py"
            } else {
                "server.py"
            };

            return Some(DetectedMcpInfo {
                name: dir_name,
                description: "基于 Python 的 MCP 服务".to_string(),
                transport: McpTransport::Stdio,
                command: Some("uv".to_string()),
                args: vec!["run".to_string(), server_script.to_string()],
                env: HashMap::new(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_various_github_urls() {
        // 完整 URL
        let parsed = parse_git_url("https://github.com/owner/my-skill").unwrap();
        assert_eq!(parsed.owner, "owner");
        assert_eq!(parsed.repo, "my-skill");
        assert_eq!(parsed.clone_url, "https://github.com/owner/my-skill.git");
        assert!(parsed.branch.is_none());
        assert!(parsed.subpath.is_none());

        // 带 .git
        let parsed = parse_git_url("https://github.com/owner/my-skill.git").unwrap();
        assert_eq!(parsed.repo, "my-skill");

        // 带 /tree/main/sub/dir
        let parsed = parse_git_url("https://github.com/owner/repo/tree/main/skills/weather").unwrap();
        assert_eq!(parsed.owner, "owner");
        assert_eq!(parsed.repo, "repo");
        assert_eq!(parsed.branch.as_deref(), Some("main"));
        assert_eq!(parsed.subpath.as_deref(), Some("skills/weather"));

        // 带 /blob/v1.0/skills/demo/SKILL.md
        let parsed = parse_git_url("https://github.com/owner/repo/blob/v1.0/skills/demo/SKILL.md").unwrap();
        assert_eq!(parsed.branch.as_deref(), Some("v1.0"));
        assert_eq!(parsed.subpath.as_deref(), Some("skills/demo"));

        // 简写 owner/repo
        let parsed = parse_git_url("anthropics/anthropic-quickstarts").unwrap();
        assert_eq!(parsed.owner, "anthropics");
        assert_eq!(parsed.repo, "anthropic-quickstarts");
        assert_eq!(
            parsed.clone_url,
            "https://github.com/anthropics/anthropic-quickstarts.git"
        );

        // SSH 形式
        let parsed = parse_git_url("git@github.com:foo/bar.git").unwrap();
        assert_eq!(parsed.owner, "foo");
        assert_eq!(parsed.repo, "bar");
    }

    #[test]
    fn inspect_detects_skill_and_mcp() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        // 构造单技能
        let skill_dir = root.join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Test Skill\ndescription: Test description\nversion: 1.0.0\n---\n# Docs",
        )
        .unwrap();

        let (skills, mcps, kind) = inspect_directory(root, None).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].title, "Test Skill");
        assert_eq!(kind, RemoteSourceType::SingleSkill);
        assert!(mcps.is_empty());
    }

    #[test]
    fn inspect_detects_mcp_package_json() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        fs::write(
            root.join("package.json"),
            r#"{"name": "mcp-test", "description": "Test server", "dependencies": {"@modelcontextprotocol/sdk": "^1.0.0"}}"#,
        )
        .unwrap();

        let (skills, mcps, kind) = inspect_directory(root, None).unwrap();
        assert!(skills.is_empty());
        assert_eq!(mcps.len(), 1);
        assert_eq!(mcps[0].name, "mcp-test");
        assert_eq!(kind, RemoteSourceType::McpServer);
    }
}
