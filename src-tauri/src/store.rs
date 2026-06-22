use crate::{
    error::{AppError, AppResult},
    models::{AgentProfile, CatalogSource, DiscoveryPathEntry},
};

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::Mutex,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppState {
    #[serde(default)]
    agents: Vec<AgentProfile>,
    #[serde(default)]
    installs: Vec<InstallRecord>,
    #[serde(default)]
    discovery_paths: Vec<DiscoveryPathEntry>,
    #[serde(default)]
    operations: Vec<OperationRecord>,
    #[serde(default)]
    next_operation_id: i64,
    #[serde(default)]
    no_full_coverage_titles: HashSet<String>,
    #[serde(default)]
    no_full_coverage_mcp_titles: HashSet<String>,
    #[serde(default)]
    catalog_sources: Vec<CatalogSource>,
    #[serde(default)]
    skill_tags: HashMap<String, Vec<String>>,
    #[serde(default)]
    agent_tags: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallRecord {
    agent_id: String,
    skill_id: String,
    fingerprint: String,
    target_path: String,
    installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OperationRecord {
    id: i64,
    agent_id: String,
    skill_id: String,
    action: String,
    target_path: String,
    backup_path: Option<String>,
    created_at: String,
}

pub struct AppStore {
    state: Mutex<AppState>,
    state_path: PathBuf,
    data_dir: PathBuf,
}

impl AppStore {
    pub fn new() -> AppResult<Self> {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("skill-sync-manager");
        fs::create_dir_all(&data_dir)?;
        let state_path = data_dir.join("state.json");
        let state = if state_path.exists() {
            let text = fs::read_to_string(&state_path)?;
            serde_json::from_str(&text).unwrap_or_default()
        } else {
            AppState::default()
        };
        Ok(Self {
            state: Mutex::new(state),
            state_path,
            data_dir,
        })
    }

    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let data_dir = std::env::temp_dir().join("skill-sync-manager-test");
        Ok(Self {
            state: Mutex::new(AppState::default()),
            state_path: data_dir.join("state.json"),
            data_dir,
        })
    }

    fn save(&self) -> AppResult<()> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&*state)?;
        fs::write(&self.state_path, json)?;
        Ok(())
    }

    pub fn backup_root(&self) -> PathBuf {
        self.data_dir.join("backups")
    }

    pub fn import_root(&self) -> PathBuf {
        self.data_dir.join("imports")
    }

    pub fn data_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }

    pub fn save_agent(&self, profile: &AgentProfile) -> AppResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        if let Some(existing) = state.agents.iter_mut().find(|a| a.id == profile.id) {
            *existing = profile.clone();
        } else {
            state.agents.push(profile.clone());
        }
        drop(state);
        self.save()
    }

    pub fn remove_agent(&self, agent_id: &str) -> AppResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        state.agents.retain(|a| a.id != agent_id);
        state.installs.retain(|i| i.agent_id != agent_id);
        state.agent_tags.remove(agent_id);
        drop(state);
        self.save()
    }

    pub fn list_agents(&self) -> AppResult<Vec<AgentProfile>> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let mut agents = state.agents.clone();
        agents.sort_by(|a, b| a.name.cmp(&b.name).then(a.skills_path.cmp(&b.skills_path)));
        Ok(agents)
    }

    pub fn installed_fingerprint(
        &self,
        agent_id: &str,
        skill_id: &str,
    ) -> AppResult<Option<String>> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        Ok(state
            .installs
            .iter()
            .find(|i| i.agent_id == agent_id && i.skill_id == skill_id)
            .map(|i| i.fingerprint.clone()))
    }

    pub fn record_install(
        &self,
        agent_id: &str,
        skill_id: &str,
        fingerprint: &str,
        target_path: &str,
        action: &str,
        backup_path: Option<&str>,
    ) -> AppResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let now = chrono::Utc::now().to_rfc3339();

        if let Some(existing) = state
            .installs
            .iter_mut()
            .find(|i| i.agent_id == agent_id && i.skill_id == skill_id)
        {
            existing.fingerprint = fingerprint.to_string();
            existing.target_path = target_path.to_string();
            existing.installed_at = now.clone();
        } else {
            state.installs.push(InstallRecord {
                agent_id: agent_id.to_string(),
                skill_id: skill_id.to_string(),
                fingerprint: fingerprint.to_string(),
                target_path: target_path.to_string(),
                installed_at: now.clone(),
            });
        }

        let op_id = state.next_operation_id;
        state.next_operation_id += 1;
        state.operations.push(OperationRecord {
            id: op_id,
            agent_id: agent_id.to_string(),
            skill_id: skill_id.to_string(),
            action: action.to_string(),
            target_path: target_path.to_string(),
            backup_path: backup_path.map(|s| s.to_string()),
            created_at: now,
        });

        drop(state);
        self.save()
    }

    pub fn record_uninstall(
        &self,
        agent_id: &str,
        skill_id: &str,
        target_path: &str,
        backup_path: Option<&str>,
    ) -> AppResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let now = chrono::Utc::now().to_rfc3339();

        state
            .installs
            .retain(|i| !(i.agent_id == agent_id && i.skill_id == skill_id));

        let op_id = state.next_operation_id;
        state.next_operation_id += 1;
        state.operations.push(OperationRecord {
            id: op_id,
            agent_id: agent_id.to_string(),
            skill_id: skill_id.to_string(),
            action: "uninstall".to_string(),
            target_path: target_path.to_string(),
            backup_path: backup_path.map(|s| s.to_string()),
            created_at: now,
        });

        drop(state);
        self.save()
    }

    pub fn last_backup(
        &self,
        agent_id: &str,
        skill_id: &str,
    ) -> AppResult<Option<(String, String)>> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        Ok(state
            .operations
            .iter()
            .rev()
            .find(|op| {
                op.agent_id == agent_id && op.skill_id == skill_id && op.backup_path.is_some()
            })
            .map(|op| (op.target_path.clone(), op.backup_path.clone().unwrap())))
    }

    pub fn toggle_no_full_coverage(&self, title: &str) -> AppResult<bool> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let is_now_marked = if state.no_full_coverage_titles.contains(title) {
            state.no_full_coverage_titles.remove(title);
            false
        } else {
            state.no_full_coverage_titles.insert(title.to_string());
            true
        };
        drop(state);
        self.save()?;
        Ok(is_now_marked)
    }

    pub fn list_no_full_coverage(&self) -> AppResult<Vec<String>> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        Ok(state.no_full_coverage_titles.iter().cloned().collect())
    }

    pub fn toggle_no_full_coverage_mcp(&self, title: &str) -> AppResult<bool> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let is_now_marked = if state.no_full_coverage_mcp_titles.contains(title) {
            state.no_full_coverage_mcp_titles.remove(title);
            false
        } else {
            state.no_full_coverage_mcp_titles.insert(title.to_string());
            true
        };
        drop(state);
        self.save()?;
        Ok(is_now_marked)
    }

    pub fn list_no_full_coverage_mcp(&self) -> AppResult<Vec<String>> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        Ok(state.no_full_coverage_mcp_titles.iter().cloned().collect())
    }

    pub fn set_skill_tags(&self, title: &str, tags: Vec<String>) -> AppResult<Vec<String>> {
        let key = normalize_skill_title(title);
        if key.is_empty() {
            return Err(AppError::Message("Skill 标题不能为空。".to_string()));
        }
        let tags = sanitize_tags(tags)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        if tags.is_empty() {
            state.skill_tags.remove(&key);
        } else {
            state.skill_tags.insert(key, tags.clone());
        }
        drop(state);
        self.save()?;
        Ok(tags)
    }

    pub fn list_skill_tags(&self, title: &str) -> AppResult<Vec<String>> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        Ok(state
            .skill_tags
            .get(&normalize_skill_title(title))
            .cloned()
            .unwrap_or_default())
    }

    pub fn set_agent_tags(&self, agent_id: &str, tags: Vec<String>) -> AppResult<Vec<String>> {
        let key = agent_id.trim();
        if key.is_empty() {
            return Err(AppError::Message("Agent ID 不能为空。".to_string()));
        }
        let tags = sanitize_tags(tags)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        if tags.is_empty() {
            state.agent_tags.remove(key);
        } else {
            state.agent_tags.insert(key.to_string(), tags.clone());
        }
        drop(state);
        self.save()?;
        Ok(tags)
    }

    pub fn list_agent_tags(&self, agent_id: &str) -> AppResult<Vec<String>> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        Ok(state
            .agent_tags
            .get(agent_id.trim())
            .cloned()
            .unwrap_or_default())
    }

    pub fn save_catalog_source(&self, source: &CatalogSource) -> AppResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        if let Some(existing) = state
            .catalog_sources
            .iter_mut()
            .find(|item| item.id == source.id)
        {
            *existing = source.clone();
        } else {
            state.catalog_sources.push(source.clone());
        }
        drop(state);
        self.save()
    }

    pub fn list_catalog_sources(&self) -> AppResult<Vec<CatalogSource>> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let mut sources = state.catalog_sources.clone();
        sources.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(sources)
    }
}

fn normalize_skill_title(title: &str) -> String {
    title.trim().to_lowercase()
}

fn sanitize_tags(tags: Vec<String>) -> AppResult<Vec<String>> {
    let mut seen = HashSet::new();
    let mut cleaned = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > 32 {
            return Err(AppError::Message(format!(
                "标签不能超过 32 个字符: {}",
                trimmed
            )));
        }
        let normalized = trimmed.to_lowercase();
        if seen.insert(normalized) {
            cleaned.push(trimmed.to_string());
        }
    }
    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentType, CatalogSource, CatalogSourceKind};

    #[test]
    fn saves_and_lists_agents() {
        let store = AppStore::in_memory().unwrap();
        let profile = AgentProfile {
            id: "test-1".into(),
            name: "Test Agent".into(),
            agent_type: AgentType::Custom,
            skills_path: "/tmp/test".into(),
            adapter_config: None,
            user_tags: Vec::new(),
        };
        store.save_agent(&profile).unwrap();
        let agents = store.list_agents().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, "test-1");
    }

    #[test]
    fn removes_agent_and_installs() {
        let store = AppStore::in_memory().unwrap();
        let profile = AgentProfile {
            id: "test-1".into(),
            name: "Test".into(),
            agent_type: AgentType::Custom,
            skills_path: "/tmp/test".into(),
            adapter_config: None,
            user_tags: Vec::new(),
        };
        store.save_agent(&profile).unwrap();
        store
            .record_install("test-1", "skill-a", "fp1", "/target", "installed", None)
            .unwrap();
        store.remove_agent("test-1").unwrap();
        assert!(store.list_agents().unwrap().is_empty());
        assert!(store
            .installed_fingerprint("test-1", "skill-a")
            .unwrap()
            .is_none());
    }

    #[test]
    fn records_and_queries_install() {
        let store = AppStore::in_memory().unwrap();
        store
            .record_install("a1", "s1", "fp123", "/target/path", "installed", None)
            .unwrap();
        assert_eq!(
            store.installed_fingerprint("a1", "s1").unwrap(),
            Some("fp123".to_string())
        );
    }

    #[test]
    fn saves_and_lists_catalog_sources() {
        let store = AppStore::in_memory().unwrap();
        let source = CatalogSource {
            id: "custom-demo".into(),
            name: "Custom Demo".into(),
            url: "https://example.com/demo.git".into(),
            kind: CatalogSourceKind::Custom,
            icon: "custom".into(),
            enabled: true,
            last_refreshed_at: None,
            cache_path: None,
        };

        store.save_catalog_source(&source).unwrap();

        let sources = store.list_catalog_sources().unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, "custom-demo");
    }

    #[test]
    fn saves_sanitized_skill_tags_by_normalized_title() {
        let store = AppStore::in_memory().unwrap();

        store
            .set_skill_tags(
                "  Demo Skill  ",
                vec![
                    " AI ".to_string(),
                    "ai".to_string(),
                    "".to_string(),
                    "中文标签".to_string(),
                ],
            )
            .unwrap();

        let tags = store.list_skill_tags("demo skill").unwrap();
        assert_eq!(tags, vec!["AI".to_string(), "中文标签".to_string()]);
    }

    #[test]
    fn saves_sanitized_agent_tags_by_agent_id() {
        let store = AppStore::in_memory().unwrap();

        store
            .set_agent_tags(
                "agent-1",
                vec![
                    " 生产力 ".to_string(),
                    "生产力".to_string(),
                    "".to_string(),
                    "AI".to_string(),
                ],
            )
            .unwrap();

        let tags = store.list_agent_tags("agent-1").unwrap();
        assert_eq!(tags, vec!["生产力".to_string(), "AI".to_string()]);
    }

    #[test]
    fn rejects_agent_tags_longer_than_thirty_two_chars() {
        let store = AppStore::in_memory().unwrap();

        let result = store.set_agent_tags("agent-1", vec!["a".repeat(33)]);

        assert!(result.is_err());
    }

    #[test]
    fn empty_agent_tags_clear_existing_tags() {
        let store = AppStore::in_memory().unwrap();
        store
            .set_agent_tags("agent-1", vec!["测试".to_string()])
            .unwrap();

        store.set_agent_tags("agent-1", Vec::new()).unwrap();

        assert!(store.list_agent_tags("agent-1").unwrap().is_empty());
    }
}
