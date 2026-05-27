pub mod adapter;
mod commands;
pub mod error;
pub mod hash;
pub mod manifest;
pub mod models;
pub mod native_app;
pub mod service;
pub mod store;

use commands::*;
use service::AppService;

pub fn run() {
    tauri::Builder::default()
        .manage(AppService::new().expect("failed to initialize local service"))
        .invoke_handler(tauri::generate_handler![
            set_repository,
            get_repository,
            scan_skills,
            import_skill_upload,
            detect_agents,
            list_agents,
            add_agent,
            remove_agent,
            list_install_state,
            scan_agent_skills,
            preview_sync,
            install_skills,
            sync_grouped_skill,
            uninstall_skill,
            rollback_last
        ])
        .run(tauri::generate_context!())
        .expect("error while running Skills Manager");
}

pub fn run_native() -> eframe::Result<()> {
    native_app::run()
}
