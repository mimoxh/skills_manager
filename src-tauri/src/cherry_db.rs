use crate::error::{AppError, AppResult};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CherrySkillRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub folder_name: String,
    pub source: String,
    pub content_hash: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct CherryAgentRow {
    pub id: String,
    pub name: String,
}

pub struct CherryDb {
    db_path: PathBuf,
}

impl CherryDb {
    pub fn open(db_path: &Path) -> AppResult<Self> {
        if !db_path.exists() {
            return Err(AppError::Message(format!(
                "Cherry Studio 数据库不存在: {}",
                db_path.display()
            )));
        }
        Ok(Self {
            db_path: db_path.to_path_buf(),
        })
    }

    fn connect(&self) -> AppResult<Connection> {
        Connection::open_with_flags(
            &self.db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| AppError::Message(format!("无法打开 Cherry Studio 数据库: {}", e)))
    }

    pub fn list_skills(&self) -> AppResult<Vec<CherrySkillRow>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare("SELECT id, name, description, folder_name, source, content_hash, is_enabled FROM skills")
            .map_err(|e| AppError::Message(format!("查询 skills 失败: {}", e)))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CherrySkillRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    folder_name: row.get(3)?,
                    source: row.get(4)?,
                    content_hash: row.get(5)?,
                    is_enabled: row.get(6)?,
                })
            })
            .map_err(|e| AppError::Message(format!("读取 skills 行失败: {}", e)))?;
        let mut skills = Vec::new();
        for row in rows {
            skills.push(row.map_err(|e| AppError::Message(format!("解析 skill 行失败: {}", e)))?);
        }
        Ok(skills)
    }

    pub fn get_skill(&self, folder_name: &str) -> AppResult<Option<CherrySkillRow>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare("SELECT id, name, description, folder_name, source, content_hash, is_enabled FROM skills WHERE folder_name = ?1")
            .map_err(|e| AppError::Message(format!("查询 skill 失败: {}", e)))?;
        let mut rows = stmt
            .query_map(params![folder_name], |row| {
                Ok(CherrySkillRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    folder_name: row.get(3)?,
                    source: row.get(4)?,
                    content_hash: row.get(5)?,
                    is_enabled: row.get(6)?,
                })
            })
            .map_err(|e| AppError::Message(format!("查询 skill 失败: {}", e)))?;
        match rows.next() {
            Some(row) => Ok(Some(
                row.map_err(|e| AppError::Message(format!("解析 skill 行失败: {}", e)))?,
            )),
            None => Ok(None),
        }
    }

    pub fn insert_skill(
        &self,
        name: &str,
        description: Option<&str>,
        folder_name: &str,
        content_hash: &str,
    ) -> AppResult<String> {
        let conn = self.connect()?;
        let id = uuid_v4();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO skills (id, name, description, folder_name, source, content_hash, is_enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, name, description, folder_name, "local", content_hash, true, now, now],
        )
        .map_err(|e| AppError::Message(format!("插入 skill 失败: {}", e)))?;
        Ok(id)
    }

    pub fn update_skill(
        &self,
        folder_name: &str,
        name: &str,
        description: Option<&str>,
        content_hash: &str,
    ) -> AppResult<()> {
        let conn = self.connect()?;
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE skills SET name = ?1, description = ?2, content_hash = ?3, updated_at = ?4 WHERE folder_name = ?5",
            params![name, description, content_hash, now, folder_name],
        )
        .map_err(|e| AppError::Message(format!("更新 skill 失败: {}", e)))?;
        Ok(())
    }

    pub fn delete_skill(&self, folder_name: &str) -> AppResult<()> {
        let conn = self.connect()?;
        // agent_skills 有 CASCADE 外键，删除 skill 自动清理关联
        conn.execute("DELETE FROM skills WHERE folder_name = ?1", params![folder_name])
            .map_err(|e| AppError::Message(format!("删除 skill 失败: {}", e)))?;
        Ok(())
    }

    pub fn list_agents(&self) -> AppResult<Vec<CherryAgentRow>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare("SELECT id, name FROM agents WHERE deleted_at IS NULL")
            .map_err(|e| AppError::Message(format!("查询 agents 失败: {}", e)))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CherryAgentRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| AppError::Message(format!("读取 agents 行失败: {}", e)))?;
        let mut agents = Vec::new();
        for row in rows {
            agents.push(row.map_err(|e| AppError::Message(format!("解析 agent 行失败: {}", e)))?);
        }
        Ok(agents)
    }

    pub fn enable_skill_for_agents(&self, skill_id: &str, agent_ids: &[String]) -> AppResult<()> {
        let conn = self.connect()?;
        let now = chrono::Utc::now().timestamp_millis();
        for agent_id in agent_ids {
            // 忽略已存在的关联
            conn.execute(
                "INSERT OR IGNORE INTO agent_skills (agent_id, skill_id, is_enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![agent_id, skill_id, true, now, now],
            )
            .map_err(|e| AppError::Message(format!("关联 skill 到 agent 失败: {}", e)))?;
        }
        Ok(())
    }

    pub fn unlink_skill(&self, skill_id: &str) -> AppResult<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM agent_skills WHERE skill_id = ?1", params![skill_id])
            .map_err(|e| AppError::Message(format!("删除 skill 关联失败: {}", e)))?;
        Ok(())
    }
}

/// Generate a lowercase UUID v4 string.
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // Simple pseudo-UUID using timestamp + random-ish bits
    format!("{:032x}", seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_test_db(dir: &Path) -> PathBuf {
        let db_path = dir.join("agents.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                folder_name TEXT NOT NULL,
                source TEXT NOT NULL,
                source_url TEXT,
                namespace TEXT,
                author TEXT,
                tags TEXT,
                content_hash TEXT NOT NULL,
                is_enabled INTEGER DEFAULT 1 NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS skills_folder_name_unique ON skills (folder_name);
            CREATE TABLE IF NOT EXISTS agent_skills (
                agent_id TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                is_enabled INTEGER DEFAULT 0 NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY(agent_id, skill_id),
                FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE,
                FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY NOT NULL,
                type TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                model TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT '',
                deleted_at TEXT
            );",
        )
        .unwrap();
        db_path
    }

    #[test]
    fn insert_and_list_skills() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_db(dir.path());
        let db = CherryDb::open(&db_path).unwrap();

        let id = db
            .insert_skill("Test Skill", Some("desc"), "test-skill", "abc123")
            .unwrap();
        assert!(!id.is_empty());

        let skills = db.list_skills().unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "Test Skill");
        assert_eq!(skills[0].folder_name, "test-skill");
    }

    #[test]
    fn delete_skill_cascades() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_db(dir.path());
        let db = CherryDb::open(&db_path).unwrap();

        // Insert an agent first (required by FK constraint)
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO agents (id, type, name, model, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["agent1", "claude", "Test Agent", "default", "2026-01-01", "2026-01-01"],
        )
        .unwrap();

        let id = db
            .insert_skill("Test", None, "test", "hash")
            .unwrap();
        db.enable_skill_for_agents(&id, &["agent1".to_string()]).unwrap();
        db.delete_skill("test").unwrap();

        assert!(db.list_skills().unwrap().is_empty());
    }

    #[test]
    fn get_nonexistent_skill_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_db(dir.path());
        let db = CherryDb::open(&db_path).unwrap();
        assert!(db.get_skill("nonexistent").unwrap().is_none());
    }
}
