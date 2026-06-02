pub mod adapter;
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
pub mod store;

use commands::*;
use service::AppService;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppService::new().expect("failed to initialize local service"))
        .invoke_handler(tauri::generate_handler![
            get_initial_data,
            import_skill_upload,
            detect_agents,
            list_agents,
            add_agent,
            remove_agent,
            scan_agent_skills,
            sync_grouped_skill,
            uninstall_skill,
            uninstall_skill_from_agents,
            rollback_last,
            toggle_no_full_coverage,
            toggle_no_full_coverage_mcp,
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
