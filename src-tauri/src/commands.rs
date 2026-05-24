use crate::{
    adapter::{adapter_for, built_in_adapters, AgentAdapter},
    error::{AppError, AppResult},
    hash::copy_dir_all,
    manifest::scan_repository,
    models::{AgentProfile, ConflictPolicy, InstallState, SkillSummary, SyncCandidate},
    store::AppStore,
};
use std::{collections::HashMap, fs, path::Path};
use tauri::State;

fn load_skills(store: &AppStore) -> AppResult<Vec<SkillSummary>> {
    let repository = store
        .get_repository()?
        .ok_or_else(|| AppError::Message("No skills repository configured".to_string()))?;
    scan_repository(Path::new(&repository))
}

fn load_agents(store: &AppStore) -> AppResult<Vec<AgentProfile>> {
    let mut agents = HashMap::new();
    for agent in store.list_agents()? {
        agents.insert(agent.id.clone(), agent);
    }
    for adapter in built_in_adapters() {
        for agent in adapter.detect() {
            agents.entry(agent.id.clone()).or_insert(agent);
        }
    }
    Ok(agents.into_values().collect())
}

#[tauri::command]
pub fn set_repository(path: String, store: State<AppStore>) -> AppResult<String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(AppError::Message("Repository path is required".to_string()));
    }
    fs::create_dir_all(path)?;
    store.set_repository(path)?;
    Ok(path.to_string())
}

#[tauri::command]
pub fn get_repository(store: State<AppStore>) -> AppResult<Option<String>> {
    store.get_repository()
}

#[tauri::command]
pub fn scan_skills(store: State<AppStore>) -> AppResult<Vec<SkillSummary>> {
    load_skills(&store)
}

#[tauri::command]
pub fn detect_agents() -> AppResult<Vec<AgentProfile>> {
    let mut agents = Vec::new();
    for adapter in built_in_adapters() {
        agents.extend(adapter.detect());
    }
    Ok(agents)
}

#[tauri::command]
pub fn list_agents(store: State<AppStore>) -> AppResult<Vec<AgentProfile>> {
    store.list_agents()
}

#[tauri::command]
pub fn add_agent(profile: AgentProfile, store: State<AppStore>) -> AppResult<AgentProfile> {
    let adapter = adapter_for(&profile);
    adapter.validate(&profile)?;
    store.save_agent(&profile)?;
    Ok(profile)
}

#[tauri::command]
pub fn remove_agent(agent_id: String, store: State<AppStore>) -> AppResult<()> {
    store.remove_agent(&agent_id)
}

#[tauri::command]
pub fn list_install_state(store: State<AppStore>) -> AppResult<Vec<InstallState>> {
    let skills = load_skills(&store).unwrap_or_default();
    let agents = load_agents(&store)?;
    let mut states = Vec::new();
    for agent in agents {
        let adapter = adapter_for(&agent);
        for skill in &skills {
            let installed = store.installed_fingerprint(&agent.id, &skill.manifest.id)?;
            states.push(adapter.diff(skill, &agent, installed)?);
        }
    }
    Ok(states)
}

#[tauri::command]
pub fn preview_sync(agent_id: String, store: State<AppStore>) -> AppResult<Vec<SyncCandidate>> {
    let skills = load_skills(&store)?;
    let agent = load_agents(&store)?
        .into_iter()
        .find(|agent| agent.id == agent_id)
        .ok_or_else(|| AppError::Message(format!("Agent not found: {}", agent_id)))?;
    let adapter = adapter_for(&agent);
    let mut candidates = Vec::new();
    for skill in skills {
        if adapter.check_compatibility(&skill, &agent) {
            let installed = store.installed_fingerprint(&agent.id, &skill.manifest.id)?;
            let state = adapter.diff(&skill, &agent, installed)?;
            candidates.push(SyncCandidate {
                skill,
                states: vec![state],
            });
        }
    }
    Ok(candidates)
}

#[tauri::command]
pub fn install_skills(
    skill_ids: Vec<String>,
    agent_ids: Vec<String>,
    conflict_policy: ConflictPolicy,
    store: State<AppStore>,
) -> AppResult<Vec<crate::models::InstallResult>> {
    let skills = load_skills(&store)?;
    let agents = load_agents(&store)?;
    let skill_map: HashMap<_, _> = skills
        .into_iter()
        .map(|skill| (skill.manifest.id.clone(), skill))
        .collect();
    let agent_map: HashMap<_, _> = agents
        .into_iter()
        .map(|agent| (agent.id.clone(), agent))
        .collect();
    let mut results = Vec::new();

    for agent_id in agent_ids {
        let agent = agent_map
            .get(&agent_id)
            .ok_or_else(|| AppError::Message(format!("Agent not found: {}", agent_id)))?;
        let adapter = adapter_for(agent);
        for skill_id in &skill_ids {
            let skill = skill_map
                .get(skill_id)
                .ok_or_else(|| AppError::Message(format!("Skill not found: {}", skill_id)))?;
            let result =
                adapter.install(skill, agent, conflict_policy.clone(), &store.backup_root())?;
            if result.action != "skipped" {
                store.record_install(
                    &result.agent_id,
                    &result.skill_id,
                    &skill.fingerprint,
                    &result.target_path,
                    &result.action,
                    result.backup_path.as_deref(),
                )?;
            }
            results.push(result);
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn uninstall_skill(
    skill_id: String,
    agent_id: String,
    store: State<AppStore>,
) -> AppResult<()> {
    let agent = load_agents(&store)?
        .into_iter()
        .find(|agent| agent.id == agent_id)
        .ok_or_else(|| AppError::Message(format!("Agent not found: {}", agent_id)))?;
    let adapter = adapter_for(&agent);
    let backup = adapter.uninstall(&skill_id, &agent, &store.backup_root())?;
    let target_path = Path::new(&agent.skills_path)
        .join(&skill_id)
        .to_string_lossy()
        .to_string();
    store.record_uninstall(
        &agent_id,
        &skill_id,
        &target_path,
        backup
            .as_ref()
            .map(|path| path.to_string_lossy())
            .as_deref(),
    )
}

#[tauri::command]
pub fn rollback_last(agent_id: String, skill_id: String, store: State<AppStore>) -> AppResult<()> {
    let (target, backup) = store
        .last_backup(&agent_id, &skill_id)?
        .ok_or_else(|| AppError::Message("No backup found for this skill".to_string()))?;
    let target_path = Path::new(&target);
    if target_path.exists() {
        fs::remove_dir_all(target_path)?;
    }
    copy_dir_all(Path::new(&backup), target_path)?;
    Ok(())
}
