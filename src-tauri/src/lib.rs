pub mod adapter;
mod commands;
pub mod error;
pub mod hash;
pub mod manifest;
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
            rollback_last
        ])
        .run(tauri::generate_context!())
        .expect("error while running Skills Manager");
}
