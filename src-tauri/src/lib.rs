mod adapter;
mod commands;
mod error;
mod hash;
mod manifest;
mod models;
mod store;

use commands::*;
use store::AppStore;

pub fn run() {
    tauri::Builder::default()
        .manage(AppStore::new().expect("failed to initialize local store"))
        .invoke_handler(tauri::generate_handler![
            set_repository,
            get_repository,
            scan_skills,
            detect_agents,
            list_agents,
            add_agent,
            remove_agent,
            list_install_state,
            preview_sync,
            install_skills,
            uninstall_skill,
            rollback_last
        ])
        .run(tauri::generate_context!())
        .expect("error while running Skills Manager");
}
