use crate::{
    error::{AppError, AppResult},
    models::{
        AgentProfile, CatalogFilters, CatalogRefreshResult, CatalogRefreshStatus,
        CatalogSafetyMode, CatalogSearchResult, CatalogSort, CatalogSource, ConflictPolicy,
        GroupedMcpServer, GroupedSkill, ImportSkillFile, ImportSkillResult, InitialData,
        InstallResult, McpOperationResult, McpServerConfig,
    },
    service::AppService,
};
use tauri::State;

async fn run_blocking<T, F>(task: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| AppError::Message(format!("后台任务失败: {}", error)))?
}

#[tauri::command]
pub fn get_initial_data(service: State<AppService>) -> AppResult<InitialData> {
    service.get_initial_data()
}

#[tauri::command]
pub fn import_skill_upload(
    file_name: String,
    files: Vec<ImportSkillFile>,
    target_agent_ids: Vec<String>,
    conflict_policy: ConflictPolicy,
    service: State<AppService>,
) -> AppResult<ImportSkillResult> {
    service.import_uploaded_files(&file_name, &files, &target_agent_ids, conflict_policy)
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
pub fn read_agent_skill_readme(
    skill_path: String,
    service: State<AppService>,
) -> AppResult<Option<String>> {
    service.read_agent_skill_readme(&skill_path)
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
pub fn uninstall_skill_from_agents(
    skill_id: String,
    agent_ids: Vec<String>,
    service: State<AppService>,
) -> AppResult<()> {
    service.uninstall_skill_from_agents(&skill_id, &agent_ids)
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
pub fn repair_claude_cowork_manifest(
    agent_id: String,
    service: State<AppService>,
) -> AppResult<ImportSkillResult> {
    service.repair_claude_cowork_manifest(&agent_id)
}

#[tauri::command]
pub fn toggle_no_full_coverage(title: String, service: State<AppService>) -> AppResult<bool> {
    service.toggle_no_full_coverage(&title)
}

#[tauri::command]
pub fn toggle_no_full_coverage_mcp(title: String, service: State<AppService>) -> AppResult<bool> {
    service.toggle_no_full_coverage_mcp(&title)
}

#[tauri::command]
pub fn set_skill_tags(
    title: String,
    tags: Vec<String>,
    service: State<AppService>,
) -> AppResult<Vec<String>> {
    service.set_skill_tags(&title, tags)
}

#[tauri::command]
pub fn set_agent_tags(
    agent_id: String,
    tags: Vec<String>,
    service: State<AppService>,
) -> AppResult<Vec<String>> {
    service.set_agent_tags(&agent_id, tags)
}

#[tauri::command]
pub fn list_catalog_sources(service: State<AppService>) -> AppResult<Vec<CatalogSource>> {
    service.list_catalog_sources()
}

#[tauri::command]
pub fn save_catalog_source(
    source: CatalogSource,
    service: State<AppService>,
) -> AppResult<CatalogSource> {
    service.save_catalog_source(source)
}

#[tauri::command]
pub async fn refresh_catalog_source(
    source_id: String,
    service: State<'_, AppService>,
) -> AppResult<CatalogRefreshResult> {
    let service = service.inner().clone();
    run_blocking(move || service.refresh_catalog_source(&source_id)).await
}

#[tauri::command]
pub fn start_catalog_refresh(
    source_id: String,
    mode: Option<String>,
    safety_mode: CatalogSafetyMode,
    service: State<AppService>,
) -> AppResult<CatalogRefreshStatus> {
    service.start_catalog_refresh(&source_id, mode, safety_mode)
}

#[tauri::command]
pub fn get_catalog_refresh_status(
    source_id: String,
    safety_mode: CatalogSafetyMode,
    service: State<AppService>,
) -> AppResult<CatalogRefreshStatus> {
    service.get_catalog_refresh_status(&source_id, safety_mode)
}

#[tauri::command]
pub fn cancel_catalog_refresh(
    source_id: String,
    safety_mode: CatalogSafetyMode,
    service: State<AppService>,
) -> AppResult<CatalogRefreshStatus> {
    service.cancel_catalog_refresh(&source_id, safety_mode)
}

#[tauri::command]
pub async fn search_catalog_skills(
    query: Option<String>,
    sort: CatalogSort,
    filters: CatalogFilters,
    page: Option<usize>,
    page_size: Option<usize>,
    service: State<'_, AppService>,
) -> AppResult<CatalogSearchResult> {
    let service = service.inner().clone();
    run_blocking(move || {
        service.search_catalog_skills(query.as_deref(), sort, filters, page, page_size)
    })
    .await
}

#[tauri::command]
pub fn install_catalog_skill(
    catalog_skill_id: String,
    target_agent_ids: Vec<String>,
    conflict_policy: ConflictPolicy,
    service: State<AppService>,
) -> AppResult<Vec<InstallResult>> {
    service.install_catalog_skill(&catalog_skill_id, target_agent_ids, conflict_policy)
}

// ── MCP Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn scan_mcp_servers(service: State<AppService>) -> AppResult<Vec<GroupedMcpServer>> {
    let agents = service.list_agents()?;
    service.mcp().scan_mcp_servers(&agents)
}

#[tauri::command]
pub fn add_mcp_server(
    agent_ids: Vec<String>,
    config: McpServerConfig,
    conflict_policy: ConflictPolicy,
    service: State<AppService>,
) -> AppResult<Vec<McpOperationResult>> {
    let agents = service.list_agents()?;
    service
        .mcp()
        .add_mcp_server(&agents, &agent_ids, &config, conflict_policy)
}

#[tauri::command]
pub fn update_mcp_server(
    agent_id: String,
    original_name: String,
    config: McpServerConfig,
    service: State<AppService>,
) -> AppResult<McpOperationResult> {
    let agents = service.list_agents()?;
    service
        .mcp()
        .update_mcp_server(&agents, &agent_id, &original_name, &config)
}

#[tauri::command]
pub fn remove_mcp_server(
    agent_id: String,
    name: String,
    service: State<AppService>,
) -> AppResult<McpOperationResult> {
    let agents = service.list_agents()?;
    service.mcp().remove_mcp_server(&agents, &agent_id, &name)
}

#[tauri::command]
pub fn toggle_mcp_server(
    agent_id: String,
    name: String,
    disabled: bool,
    service: State<AppService>,
) -> AppResult<McpOperationResult> {
    let agents = service.list_agents()?;
    service
        .mcp()
        .toggle_mcp_server(&agents, &agent_id, &name, disabled)
}

#[tauri::command]
pub fn sync_mcp_server(
    server_name: String,
    source_agent_id: String,
    target_agent_ids: Vec<String>,
    conflict_policy: ConflictPolicy,
    service: State<AppService>,
) -> AppResult<Vec<McpOperationResult>> {
    let agents = service.list_agents()?;
    service.mcp().sync_mcp_server(
        &agents,
        &server_name,
        &source_agent_id,
        &target_agent_ids,
        conflict_policy,
    )
}

#[tauri::command]
pub fn remove_mcp_server_from_agents(
    server_name: String,
    agent_ids: Vec<String>,
    service: State<AppService>,
) -> AppResult<Vec<McpOperationResult>> {
    let agents = service.list_agents()?;
    service
        .mcp()
        .remove_mcp_server_from_agents(&agents, &server_name, &agent_ids)
}
