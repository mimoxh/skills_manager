use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OptionalExtension, params};
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
        let conn = Connection::open_with_flags(
            &self.db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        // 数据库可能正被 Cherry Studio 进程占用，设置忙等待避免 SQLITE_BUSY 立即失败
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(conn)
    }

    pub fn list_skills(&self) -> AppResult<Vec<CherrySkillRow>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, folder_name, source, content_hash, is_enabled FROM skills",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CherrySkillRow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                folder_name: row.get(3)?,
                source: row.get(4)?,
                content_hash: row.get(5)?,
                is_enabled: row.get(6)?,
            })
        })?;
        let mut skills = Vec::new();
        for row in rows {
            skills.push(row?);
        }
        Ok(skills)
    }

    pub fn get_skill(&self, folder_name: &str) -> AppResult<Option<CherrySkillRow>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, folder_name, source, content_hash, is_enabled FROM skills WHERE folder_name = ?1",
        )?;
        let mut rows = stmt.query_map(params![folder_name], |row| {
            Ok(CherrySkillRow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                folder_name: row.get(3)?,
                source: row.get(4)?,
                content_hash: row.get(5)?,
                is_enabled: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
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
        )?;
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
        )?;
        Ok(())
    }

    pub fn delete_skill(&self, folder_name: &str) -> AppResult<()> {
        let conn = self.connect()?;
        // agent_skills 有 CASCADE 外键，删除 skill 自动清理关联
        conn.execute("DELETE FROM skills WHERE folder_name = ?1", params![folder_name])?;
        Ok(())
    }

    pub fn list_agents(&self) -> AppResult<Vec<CherryAgentRow>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare("SELECT id, name FROM agents WHERE deleted_at IS NULL")?;
        let rows = stmt.query_map([], |row| {
            Ok(CherryAgentRow {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        let mut agents = Vec::new();
        for row in rows {
            agents.push(row?);
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
            )?;
        }
        Ok(())
    }

    pub fn unlink_skill(&self, skill_id: &str) -> AppResult<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM agent_skills WHERE skill_id = ?1", params![skill_id])?;
        Ok(())
    }

    /// 以单连接 + 单事务完成 skill 的插入/更新并与 agents 关联（M-B10）：
    /// 原实现 get/insert/update 与 enable_skill_for_agents 各开一条连接，中间失败会留下
    /// "有 skill 记录但未关联任何 agent" 的半完成状态。这里把 DB 部分串成事务保证原子性。
    pub fn upsert_skill_and_link_agents(
        &self,
        name: &str,
        description: Option<&str>,
        folder_name: &str,
        content_hash: &str,
        agent_ids: &[String],
    ) -> AppResult<String> {
        let mut conn = self.connect()?;
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().timestamp_millis();

        let skill_id = match tx
            .query_row(
                "SELECT id FROM skills WHERE folder_name = ?1",
                params![folder_name],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            Some(existing_id) => {
                tx.execute(
                    "UPDATE skills SET name = ?1, description = ?2, content_hash = ?3, updated_at = ?4 WHERE folder_name = ?5",
                    params![name, description, content_hash, now, folder_name],
                )?;
                existing_id
            }
            None => {
                let id = uuid_v4();
                tx.execute(
                    "INSERT INTO skills (id, name, description, folder_name, source, content_hash, is_enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![id, name, description, folder_name, "local", content_hash, true, now, now],
                )?;
                id
            }
        };

        for agent_id in agent_ids {
            tx.execute(
                "INSERT OR IGNORE INTO agent_skills (agent_id, skill_id, is_enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![agent_id, skill_id, true, now, now],
            )?;
        }
        tx.commit()?;
        Ok(skill_id)
    }
}

/// Generate a lowercase UUID v4 string.
fn uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
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

    #[test]
    fn upsert_skill_and_link_agents_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_db(dir.path());
        let db = CherryDb::open(&db_path).unwrap();
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO agents (id, type, name, model, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["agent1", "claude", "Test Agent", "default", "2026-01-01", "2026-01-01"],
        )
        .unwrap();

        // 插入并关联
        let id = db
            .upsert_skill_and_link_agents(
                "Atomic Skill",
                Some("desc"),
                "atomic-skill",
                "hash1",
                &["agent1".to_string()],
            )
            .unwrap();
        assert!(!id.is_empty());

        // 再次调用走更新分支，返回同一 id，不产生重复行
        let id2 = db
            .upsert_skill_and_link_agents(
                "Atomic Skill",
                Some("desc2"),
                "atomic-skill",
                "hash2",
                &["agent1".to_string()],
            )
            .unwrap();
        assert_eq!(id, id2);

        let skills = db.list_skills().unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].content_hash, "hash2");
        // 关联仍在（INSERT OR IGNORE 不重复）
        let count: i64 = rusqlite::Connection::open(&db_path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM agent_skills WHERE skill_id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
