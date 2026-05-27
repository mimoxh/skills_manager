use crate::{
    error::AppResult,
    models::{
        AgentProfile, ConflictPolicy, DiscoveryPathEntry, GroupedSkill, ImportSkillFile,
        ImportSkillResult, InstallResult, InstallState, SkillSummary, SyncCandidate,
    },
    service::AppService,
};
use tauri::State;

#[tauri::command]
pub fn set_repository(path: String, service: State<AppService>) -> AppResult<String> {
    service.set_repository(&path)
}

#[tauri::command]
pub fn get_repository(service: State<AppService>) -> AppResult<String> {
    service.get_repository()
}

#[tauri::command]
pub fn scan_skills(service: State<AppService>) -> AppResult<Vec<SkillSummary>> {
    service.scan_skills()
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
pub fn list_install_state(service: State<AppService>) -> AppResult<Vec<InstallState>> {
    service.list_install_state()
}

#[tauri::command]
pub fn scan_agent_skills(service: State<AppService>) -> AppResult<Vec<GroupedSkill>> {
    service.scan_agent_skills()
}

#[tauri::command]
pub fn preview_sync(agent_id: String, service: State<AppService>) -> AppResult<Vec<SyncCandidate>> {
    service.preview_sync(&agent_id)
}

#[tauri::command]
pub fn install_skills(
    skill_ids: Vec<String>,
    agent_ids: Vec<String>,
    conflict_policy: ConflictPolicy,
    service: State<AppService>,
) -> AppResult<Vec<InstallResult>> {
    service.install_skills(skill_ids, agent_ids, conflict_policy)
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

#[tauri::command]
pub fn add_discovery_path(
    path: String,
    label: String,
    skills_subdir: String,
    service: State<AppService>,
) -> AppResult<()> {
    service.add_discovery_path(&path, &label, &skills_subdir)
}

#[tauri::command]
pub fn remove_discovery_path(path: String, service: State<AppService>) -> AppResult<()> {
    service.remove_discovery_path(&path)
}

#[tauri::command]
pub fn list_discovery_paths(service: State<AppService>) -> AppResult<Vec<DiscoveryPathEntry>> {
    service.list_discovery_paths()
}
