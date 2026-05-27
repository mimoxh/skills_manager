use crate::{
    error::{AppError, AppResult},
    models::{AgentProfile, AgentType, DiscoveryPathEntry},
};
use rusqlite::{params, Connection};
use std::{fs, path::PathBuf, sync::Mutex};

pub struct AppStore {
    conn: Mutex<Connection>,
    data_dir: PathBuf,
    default_repository_path: PathBuf,
}

impl AppStore {
    pub fn new() -> AppResult<Self> {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("skill-sync-manager");
        fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("state.db");
        let conn = Connection::open(db_path)?;
        let store = Self {
            conn: Mutex::new(conn),
            default_repository_path: Self::default_repository_path(),
            data_dir,
        };
        store.migrate()?;
        Ok(store)
    }

    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let store = Self {
            conn: Mutex::new(Connection::open_in_memory()?),
            data_dir: std::env::temp_dir().join("skill-sync-manager-test"),
            default_repository_path: std::env::temp_dir()
                .join("skill-sync-manager-test")
                .join("skills"),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                agent_type TEXT NOT NULL,
                skills_path TEXT NOT NULL,
                adapter_config TEXT
            );
            CREATE TABLE IF NOT EXISTS installs (
                agent_id TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                target_path TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                PRIMARY KEY (agent_id, skill_id)
            );
            CREATE TABLE IF NOT EXISTS operations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                action TEXT NOT NULL,
                target_path TEXT NOT NULL,
                backup_path TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_discovery_paths (
                path TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                skills_subdir TEXT NOT NULL DEFAULT 'skills'
            );
            "#,
        )?;
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

    fn default_repository_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("skills")
    }

    pub fn set_repository(&self, path: &str) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        conn.execute(
            "INSERT INTO settings(key, value) VALUES('repository', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![path],
        )?;
        Ok(())
    }

    pub fn get_repository(&self) -> AppResult<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = 'repository'")?;
        let mut rows = stmt.query([])?;
        Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
    }

    pub fn get_or_create_repository(&self) -> AppResult<String> {
        if let Some(repository) = self.get_repository()? {
            fs::create_dir_all(&repository)?;
            return Ok(repository);
        }

        let repository = self.default_repository_path.to_string_lossy().to_string();
        fs::create_dir_all(&repository)?;
        self.set_repository(&repository)?;
        Ok(repository)
    }

    pub fn save_agent(&self, profile: &AgentProfile) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        conn.execute(
            r#"
            INSERT INTO agents(id, name, agent_type, skills_path, adapter_config)
            VALUES(?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                agent_type = excluded.agent_type,
                skills_path = excluded.skills_path,
                adapter_config = excluded.adapter_config
            "#,
            params![
                profile.id,
                profile.name,
                profile.agent_type.as_str(),
                profile.skills_path,
                profile
                    .adapter_config
                    .as_ref()
                    .map(|value| value.to_string())
            ],
        )?;
        Ok(())
    }

    pub fn remove_agent(&self, agent_id: &str) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        conn.execute("DELETE FROM agents WHERE id = ?1", params![agent_id])?;
        conn.execute(
            "DELETE FROM installs WHERE agent_id = ?1",
            params![agent_id],
        )?;
        Ok(())
    }

    pub fn list_agents(&self) -> AppResult<Vec<AgentProfile>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, agent_type, skills_path, adapter_config FROM agents ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            let agent_type: String = row.get(2)?;
            let adapter_config: Option<String> = row.get(4)?;
            Ok(AgentProfile {
                id: row.get(0)?,
                name: row.get(1)?,
                agent_type: match agent_type.as_str() {
                    "codex" => AgentType::Codex,
                    "claude" => AgentType::Claude,
                    "claudeCode" => AgentType::ClaudeCode,
                    "cursor" => AgentType::Cursor,
                    "windsurf" => AgentType::Windsurf,
                    "aider" => AgentType::Aider,
                    _ => AgentType::Custom,
                },
                skills_path: row.get(3)?,
                adapter_config: adapter_config.and_then(|text| serde_json::from_str(&text).ok()),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn installed_fingerprint(
        &self,
        agent_id: &str,
        skill_id: &str,
    ) -> AppResult<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let mut stmt =
            conn.prepare("SELECT fingerprint FROM installs WHERE agent_id = ?1 AND skill_id = ?2")?;
        let mut rows = stmt.query(params![agent_id, skill_id])?;
        Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
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
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        conn.execute(
            r#"
            INSERT INTO installs(agent_id, skill_id, fingerprint, target_path, installed_at)
            VALUES(?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(agent_id, skill_id) DO UPDATE SET
                fingerprint = excluded.fingerprint,
                target_path = excluded.target_path,
                installed_at = excluded.installed_at
            "#,
            params![agent_id, skill_id, fingerprint, target_path, now],
        )?;
        conn.execute(
            "INSERT INTO operations(agent_id, skill_id, action, target_path, backup_path, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![agent_id, skill_id, action, target_path, backup_path, now],
        )?;
        Ok(())
    }

    pub fn record_uninstall(
        &self,
        agent_id: &str,
        skill_id: &str,
        target_path: &str,
        backup_path: Option<&str>,
    ) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        conn.execute(
            "DELETE FROM installs WHERE agent_id = ?1 AND skill_id = ?2",
            params![agent_id, skill_id],
        )?;
        conn.execute(
            "INSERT INTO operations(agent_id, skill_id, action, target_path, backup_path, created_at) VALUES(?1, ?2, 'uninstall', ?3, ?4, ?5)",
            params![agent_id, skill_id, target_path, backup_path, now],
        )?;
        Ok(())
    }

    pub fn last_backup(
        &self,
        agent_id: &str,
        skill_id: &str,
    ) -> AppResult<Option<(String, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT target_path, backup_path FROM operations WHERE agent_id = ?1 AND skill_id = ?2 AND backup_path IS NOT NULL ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![agent_id, skill_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        } else {
            Ok(None)
        }
    }

    pub fn add_discovery_path(
        &self,
        path: &str,
        label: &str,
        skills_subdir: &str,
    ) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        conn.execute(
            "INSERT INTO agent_discovery_paths(path, label, skills_subdir) VALUES(?1, ?2, ?3) ON CONFLICT(path) DO UPDATE SET label = excluded.label, skills_subdir = excluded.skills_subdir",
            params![path, label, skills_subdir],
        )?;
        Ok(())
    }

    pub fn remove_discovery_path(&self, path: &str) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        conn.execute(
            "DELETE FROM agent_discovery_paths WHERE path = ?1",
            params![path],
        )?;
        Ok(())
    }

    pub fn list_discovery_paths(&self) -> AppResult<Vec<DiscoveryPathEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Message("Store lock poisoned".to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT path, label, skills_subdir FROM agent_discovery_paths ORDER BY label",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DiscoveryPathEntry {
                path: row.get(0)?,
                label: row.get(1)?,
                skills_subdir: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_repository_setting() {
        let store = AppStore::in_memory().unwrap();
        store.set_repository("C:\\skills").unwrap();
        assert_eq!(
            store.get_repository().unwrap(),
            Some("C:\\skills".to_string())
        );
    }

    #[test]
    fn creates_default_repository_setting() {
        let store = AppStore::in_memory().unwrap();
        let repository = store.get_or_create_repository().unwrap();
        assert_eq!(store.get_repository().unwrap(), Some(repository.clone()));
        assert!(PathBuf::from(repository).ends_with("skills"));
    }
}
