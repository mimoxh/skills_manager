use crate::{
    error::{AppError, AppResult},
    models::{ConflictPolicy, RemoteInstallOptions},
    service::AppService,
};
use std::io::{self, BufRead, Write};

#[cfg(windows)]
pub fn attach_console_if_needed(cmd: Option<&str>) {
    // MCP 服务使用标准 I/O 管道通信，禁止附加到父控制台
    if cmd == Some("mcp") {
        return;
    }
    extern "system" {
        fn AttachConsole(dw_process_id: u32) -> i32;
        fn GetStdHandle(n_std_handle: u32) -> isize;
        fn SetStdHandle(n_std_handle: u32, h_handle: isize) -> i32;
        fn CreateFileW(
            lp_file_name: *const u16,
            dw_desired_access: u32,
            dw_share_mode: u32,
            lp_security_attributes: *const std::ffi::c_void,
            dw_creation_disposition: u32,
            dw_flags_and_attributes: u32,
            h_template_file: isize,
        ) -> isize;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_OUTPUT_HANDLE: u32 = (-11i32) as u32;
    const STD_ERROR_HANDLE: u32 = (-12i32) as u32;
    const STD_INPUT_HANDLE: u32 = (-10i32) as u32;

    const GENERIC_READ: u32 = 0x80000000;
    const GENERIC_WRITE: u32 = 0x40000000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;
    const INVALID_HANDLE_VALUE: isize = -1;

    unsafe {
        let stdout_h = GetStdHandle(STD_OUTPUT_HANDLE);
        // 如果当前没有标准输出句柄（GUI 子系统默认行为），尝试附加到父控制台并重定向
        if stdout_h == 0 || stdout_h == INVALID_HANDLE_VALUE {
            if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
                let conout_name: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
                let conin_name: Vec<u16> = "CONIN$\0".encode_utf16().collect();

                let conout = CreateFileW(
                    conout_name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    0,
                );
                if conout != INVALID_HANDLE_VALUE {
                    SetStdHandle(STD_OUTPUT_HANDLE, conout);
                    SetStdHandle(STD_ERROR_HANDLE, conout);
                }

                let conin = CreateFileW(
                    conin_name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    0,
                );
                if conin != INVALID_HANDLE_VALUE {
                    SetStdHandle(STD_INPUT_HANDLE, conin);
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub fn attach_console_if_needed(_cmd: Option<&str>) {}

pub fn run(service: AppService, args: &[String]) -> AppResult<()> {
    let first_arg = args.first().map(|s| s.as_str());
    attach_console_if_needed(first_arg);

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" || args[0] == "help" {
        print_help();
        return Ok(());
    }

    let cmd = args[0].as_str();
    let rest = &args[1..];

    match cmd {
        "mcp" => {
            crate::mcp_server::run_stdio(service)?;
        }
        "list-agents" | "agents" => {
            let agents = service.list_agents()?;
            println!("已配置的 Agent 列表 (共 {} 个):", agents.len());
            println!("{:<15} {:<25} {:<10} {:<30}", "ID", "名称", "中枢状态", "Skills 路径");
            println!("{}", "-".repeat(80));
            for a in agents {
                let is_hub = a.agent_type == crate::models::AgentType::Universal
                    || crate::util::is_universal_skills_path(&a.skills_path);
                let hub_str = if is_hub { "Universal 中枢" } else { "常规" };
                println!("{:<15} {:<25} {:<10} {:<30}", a.id, a.name, hub_str, a.skills_path);
            }
        }
        "list-skills" | "skills" => {
            let skills = service.scan_agent_skills()?;
            println!("已安装的 Skills 列表 (共 {} 个):", skills.len());
            println!("{:<25} {:<10} {:<20} {:<25}", "标题", "版本", "中枢托管", "已安装 Agent");
            println!("{}", "-".repeat(80));
            for s in skills {
                let ver = s.best_copy.version.unwrap_or_else(|| "1.0.0".to_string());
                let hub_str = if s.is_universal { "是" } else { "否" };
                let agents_str = s.installed_agent_ids.join(", ");
                println!("{:<25} {:<10} {:<20} {:<25}", s.title, ver, hub_str, agents_str);
            }
        }
        "inspect" => {
            if rest.is_empty() {
                return Err(AppError::Message("用法: skills-manager inspect <github-url 或本地路径>".into()));
            }
            let url = &rest[0];
            println!("正在探测来源: {}", url);
            let insp = service.inspect_remote_source(url)?;
            println!("仓库名称: {}", insp.repo_name);
            println!("检测类型: {:?}", insp.detected_type);
            if !insp.skills.is_empty() {
                println!("\n包含 Skills ({} 个):", insp.skills.len());
                for s in &insp.skills {
                    println!("  - {} (版本: {})", s.title, s.version.as_deref().unwrap_or("1.0.0"));
                    if !s.description.is_empty() {
                        println!("    简介: {}", s.description);
                    }
                }
            }
            if !insp.mcp_servers.is_empty() {
                println!("\n包含 MCP 服务 ({} 个):", insp.mcp_servers.len());
                for m in &insp.mcp_servers {
                    println!("  - {} (命令: {:?} {:?})", m.name, m.command, m.args);
                }
            }
        }
        "install" => {
            if rest.is_empty() {
                return Err(AppError::Message("用法: skills-manager install <source> [--target <agent_id...>]".into()));
            }
            let source = &rest[0];
            let mut target_agents = Vec::new();
            let mut conflict_policy = ConflictPolicy::BackupOverwrite;
            let mut non_interactive = false;

            let mut idx = 1;
            while idx < rest.len() {
                match rest[idx].as_str() {
                    "--target" | "-t" => {
                        idx += 1;
                        while idx < rest.len() && !rest[idx].starts_with('-') {
                            target_agents.push(rest[idx].clone());
                            idx += 1;
                        }
                        continue;
                    }
                    "--to-hub" => {}
                    "--no-hub" => {
                        return Err(AppError::Message("已取消仅本机安装；Skill 必须保存到中枢。".into()));
                    }
                    "--conflict" => {
                        idx += 1;
                        if idx < rest.len() {
                            conflict_policy = match rest[idx].as_str() {
                                "skip" => ConflictPolicy::Skip,
                                "rename" => ConflictPolicy::Rename,
                                _ => ConflictPolicy::BackupOverwrite,
                            };
                        }
                    }
                    "--non-interactive" | "-y" => {
                        non_interactive = true;
                    }
                    _ => {}
                }
                idx += 1;
            }

            // 如果未指定 target_agent_ids，进行终端提问或探测
            if target_agents.is_empty() {
                let inspection = service.inspect_remote_source(source)?;
                println!("已识别到来自 '{}' 的技能:", source);
                for s in &inspection.skills {
                    println!("  - {} (v{})", s.title, s.version.as_deref().unwrap_or("1.0.0"));
                }

                if non_interactive {
                    println!("未指定目标 Agent：仅保存到中枢。补充 --target 可创建 Agent 链接。");
                } else {
                    println!("\n请选择要安装到的 Agent 目录：");
                    for (i, a) in inspection.available_agents.iter().enumerate() {
                        let hint = if a.is_hub { " [推荐: Universal 中枢]" } else { "" };
                        println!("  [{}] {} ({}){}", i + 1, a.name, a.id, hint);
                    }
                    print!("输入序号 (多个以逗号分隔，直接回车仅保存到中枢): ");
                    io::stdout().flush().ok();

                    let mut input = String::new();
                    let stdin = io::stdin();
                    stdin.lock().read_line(&mut input).ok();
                    let input_trimmed = input.trim();

                    if !input_trimmed.is_empty() {
                        for part in input_trimmed.split(',') {
                            if let Ok(num) = part.trim().parse::<usize>() {
                                if num >= 1 && num <= inspection.available_agents.len() {
                                    target_agents.push(inspection.available_agents[num - 1].id.clone());
                                }
                            } else {
                                // 允许直接输入 agent id
                                target_agents.push(part.trim().to_string());
                            }
                        }
                    }
                }
            }

            println!("正在安装到: {:?} ...", target_agents);
            let res = service.install_remote_source(RemoteInstallOptions {
                url: source.to_string(),
                target_agent_ids: target_agents,
                conflict_policy,
                to_hub: Some(true),
                selected_skills: vec![],
            })?;

            println!("✓ {}", res.message);
            for r in res.results {
                let method = r
                    .distribution_method
                    .as_deref()
                    .map(|value| format!(" / {value}"))
                    .unwrap_or_default();
                println!("  [{}{}] {} -> {}", r.action, method, r.skill_id, r.target_path);
                println!("    {}", r.message);
            }
        }
        "uninstall" => {
            if rest.is_empty() {
                return Err(AppError::Message("用法: skills-manager uninstall <skill_id>".into()));
            }
            let skill_id = &rest[0];
            let skills = service.scan_agent_skills()?;
            let matching = skills.into_iter().find(|s| s.title == *skill_id);
            if let Some(s) = matching {
                service.uninstall_skill_from_agents(skill_id, &s.installed_agent_ids)?;
                println!("✓ 已从 {} 个 Agent 中卸载技能 '{}'", s.installed_agent_ids.len(), skill_id);
            } else {
                println!("未找到技能 '{}'，可能未安装或已卸载。", skill_id);
            }
        }
        "sync" => {
            println!("正在触发多设备云同步...");
            let status = service.sync_now()?;
            println!("✓ 同步执行完成: 设备 ID: {}", status.device_id);
            if status.pending_conflicts > 0 {
                println!("! 存在 {} 个待处理的同步冲突，请在桌面端查看并解决。", status.pending_conflicts);
            }
        }
        _ => {
            eprintln!("未知指令: '{}'", cmd);
            print_help();
            return Err(AppError::Message(format!("未知指令: {}", cmd)));
        }
    }

    Ok(())
}

fn print_help() {
    println!("Skills Manager CLI & MCP - 集中管理与同步 Agent Skills 和 MCP 服务\n");
    println!("用法:");
    println!("  skills-manager                  启动桌面 GUI 界面");
    println!("  skills-manager mcp              以 MCP Server 模式运行 (stdio 协议，供 Agent 调用)");
    println!("  skills-manager list-agents      列出所有受管理的 Agent 与中枢目录");
    println!("  skills-manager list-skills      列出所有已安装技能及状态");
    println!("  skills-manager inspect <source> 探测 GitHub 仓库或本地目录中的 Skills/MCP");
    println!("  skills-manager install <source> 安装技能（默认交互式提问目标目录，支持 GitHub URL）");
    println!("    --target <id...>              指定目标 Agent ID（先用 list-agents 查询）");
    println!("    --to-hub                      兼容旧用法；Skill 始终先保存到中枢");
    println!("    --conflict <overwrite|skip|rename> 冲突策略");
    println!("    --non-interactive, -y         非交互模式");
    println!("  skills-manager uninstall <name> 从 Agent 中卸载指定技能");
    println!("  skills-manager sync             立即触发多设备技能云同步");
}
