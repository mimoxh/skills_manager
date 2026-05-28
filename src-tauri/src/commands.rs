use crate::{
    error::AppResult,
    models::{
        AgentProfile, ConflictPolicy, GroupedSkill, ImportSkillFile, ImportSkillResult,
        InitialData, InstallResult,
    },
    service::AppService,
};
use tauri::State;

#[tauri::command]
pub fn get_initial_data(service: State<AppService>) -> AppResult<InitialData> {
    service.get_initial_data()
}

#[tauri::command]
pub fn import_skill_upload(
    file_name: String,
    files: Vec<ImportSkillFile>,
    service: State<AppService>,
) -> AppResult<ImportSkillResult> {
    service.import_uploaded_files(&file_name, &files)
}

#[tauri::command]
pub fn detect_agents(service: State<AppService>) -> AppResult<Vec<AgentProfile>> {
    service.detect_agents()
}

#[tauri::command]
pub fn list_agents(service: State<AppService>) -> AppResult<Vec<AgentProfile>> {
    service.list_saved_agents()
}

#[tauri::command]
pub fn add_agent(profile: AgentProfile, service: State<AppService>) -> AppResult<AgentProfile> {
    service.add_agent(profile)
}

#[tauri::command]
pub fn remove_agent(agent_id: String, service: State<AppService>) -> AppResult<()> {
    service.remove_agent(&agent_id)
}

#[tauri::command]
pub fn scan_agent_skills(service: State<AppService>) -> AppResult<Vec<GroupedSkill>> {
    service.scan_agent_skills()
}

#[tauri::command]
pub fn sync_grouped_skill(
    title: String,
    source_agent_id: Option<String>,
    target_agent_ids: Vec<String>,
    conflict_policy: ConflictPolicy,
    service: State<AppService>,
) -> AppResult<Vec<InstallResult>> {
    service.sync_grouped_skill(
        &title,
        source_agent_id.as_deref(),
        target_agent_ids,
        conflict_policy,
    )
}

#[tauri::command]
pub fn uninstall_skill(
    skill_id: String,
    agent_id: String,
    service: State<AppService>,
) -> AppResult<()> {
    service.uninstall_skill(&skill_id, &agent_id)
}

#[tauri::command]
pub fn rollback_last(
    agent_id: String,
    skill_id: String,
    service: State<AppService>,
) -> AppResult<()> {
    service.rollback_last(&agent_id, &skill_id)
}
