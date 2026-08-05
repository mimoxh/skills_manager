pub mod adapter;
pub mod catalog;
pub mod catalog_index;
pub mod catalog_refresh;
pub mod cherry_db;
pub mod cherry_studio;
mod commands;
pub mod error;
pub mod hash;
pub mod manifest;
pub mod mcp_adapter;
pub mod mcp_claude;
pub mod mcp_codex;
pub mod mcp_opencode;
pub mod mcp_service;
pub mod mcp_trae;
pub mod models;
pub mod service;
pub mod skill_scan;
pub mod store;
pub mod util;

use commands::*;
use service::AppService;

pub fn run() {
    // 优雅降级：state.json 损坏等初始化失败时不 panic，打印明确提示后退出，
    // 避免桌面窗口完全不弹出且无任何说明。
    let service = match AppService::new() {
        Ok(service) => service,
        Err(error) => {
            eprintln!("[skills_manager] 初始化本地服务失败: {error}");
            eprintln!(
                "[skills_manager] 请检查数据目录下的 state.json 是否损坏，可将其重命名后重新启动。"
            );
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(service)
        .invoke_handler(tauri::generate_handler![
            get_initial_data,
            import_skill_upload,
            detect_agents,
            list_agents,
            add_agent,
            remove_agent,
            scan_agent_skills,
            read_agent_skill_readme,
            sync_grouped_skill,
            uninstall_skill,
            uninstall_skill_from_agents,
            rollback_last,
            repair_claude_cowork_manifest,
            toggle_no_full_coverage,
            toggle_no_full_coverage_mcp,
            set_skill_tags,
            set_agent_tags,
            list_catalog_sources,
            save_catalog_source,
            refresh_catalog_source,
            start_catalog_refresh,
            get_catalog_refresh_status,
            cancel_catalog_refresh,
            search_catalog_skills,
            install_catalog_skill,
            scan_mcp_servers,
            add_mcp_server,
            update_mcp_server,
            remove_mcp_server,
            toggle_mcp_server,
            sync_mcp_server,
            remove_mcp_server_from_agents
        ])
        .run(tauri::generate_context!())
        .expect("error while running Skills Manager");
}
