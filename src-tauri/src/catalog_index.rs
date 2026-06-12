use crate::{
    error::{AppError, AppResult},
    models::{
        CatalogFilters, CatalogInstallStatus, CatalogRefreshStatus, CatalogSafetyMode,
        CatalogSearchResult, CatalogSkill, CatalogSort, CatalogSource,
    },
};
use rusqlite::{params, Connection};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct CatalogIndex {
    db_path: PathBuf,
}

pub struct RefreshStatePatch<'a> {
    pub source_id: &'a str,
    pub safety_mode: CatalogSafetyMode,
    pub cursor: Option<&'a str>,
    pub fetched_count: usize,
    pub generation: i64,
    pub is_running: bool,
    pub is_complete: bool,
    pub last_error: Option<&'a str>,
}

impl CatalogIndex {
    pub fn new(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let index = Self {
            db_path: data_dir.join("catalog-index.sqlite"),
        };
        index.init()?;
        Ok(index)
    }

    fn connect(&self) -> AppResult<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    fn init(&self) -> AppResult<()> {
        let conn = self.connect()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS catalog_skills (
                source_id TEXT NOT NULL,
                safety_mode TEXT NOT NULL,
                slug TEXT NOT NULL,
                id TEXT NOT NULL,
                name TEXT NOT NULL,
                source_name TEXT NOT NULL,
                source_icon TEXT NOT NULL,
                source_path TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                description TEXT,
                tags_json TEXT NOT NULL,
                supported_agents_json TEXT NOT NULL,
                published_at TEXT,
                updated_at TEXT,
                download_count INTEGER,
                install_count INTEGER,
                has_skill_md INTEGER NOT NULL,
                has_scripts INTEGER NOT NULL,
                has_references INTEGER NOT NULL,
                has_assets INTEGER NOT NULL,
                generation INTEGER NOT NULL,
                PRIMARY KEY (source_id, safety_mode, slug)
            );
            CREATE INDEX IF NOT EXISTS idx_catalog_skills_source_mode_name
                ON catalog_skills(source_id, safety_mode, name);
            CREATE INDEX IF NOT EXISTS idx_catalog_skills_source_mode_downloads
                ON catalog_skills(source_id, safety_mode, download_count);
            CREATE TABLE IF NOT EXISTS catalog_refresh_state (
                source_id TEXT NOT NULL,
                safety_mode TEXT NOT NULL,
                next_cursor TEXT,
                fetched_count INTEGER NOT NULL DEFAULT 0,
                generation INTEGER NOT NULL DEFAULT 0,
                is_running INTEGER NOT NULL DEFAULT 0,
                is_complete INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                updated_at TEXT,
                PRIMARY KEY (source_id, safety_mode)
            );
            "#,
        )?;
        Ok(())
    }

    pub fn begin_refresh(&self, source_id: &str, safety_mode: CatalogSafetyMode) -> AppResult<i64> {
        let current = self.refresh_status(source_id, safety_mode)?;
        let generation = if current.is_complete {
            current.generation + 1
        } else {
            current.generation.max(1)
        };
        self.save_refresh_state(RefreshStatePatch {
            source_id,
            safety_mode,
            cursor: if current.is_complete {
                None
            } else {
                current.next_cursor.as_deref()
            },
            fetched_count: if current.is_complete {
                0
            } else {
                current.fetched_count
            },
            generation,
            is_running: true,
            is_complete: false,
            last_error: None,
        })?;
        Ok(generation)
    }

    pub fn save_refresh_state(&self, patch: RefreshStatePatch<'_>) -> AppResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO catalog_refresh_state
                (source_id, safety_mode, next_cursor, fetched_count, generation, is_running, is_complete, last_error, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(source_id, safety_mode) DO UPDATE SET
                next_cursor=excluded.next_cursor,
                fetched_count=excluded.fetched_count,
                generation=excluded.generation,
                is_running=excluded.is_running,
                is_complete=excluded.is_complete,
                last_error=excluded.last_error,
                updated_at=excluded.updated_at
            "#,
            params![
                patch.source_id,
                patch.safety_mode.as_str(),
                patch.cursor,
                patch.fetched_count as i64,
                patch.generation,
                patch.is_running as i64,
                patch.is_complete as i64,
                patch.last_error,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn refresh_status(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
    ) -> AppResult<CatalogRefreshStatus> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT next_cursor, fetched_count, generation, is_running, is_complete, last_error, updated_at
            FROM catalog_refresh_state
            WHERE source_id = ?1 AND safety_mode = ?2
            "#,
        )?;
        let mut rows = stmt.query(params![source_id, safety_mode.as_str()])?;
        if let Some(row) = rows.next()? {
            return Ok(CatalogRefreshStatus {
                source_id: source_id.to_string(),
                safety_mode,
                is_running: row.get::<_, i64>(3)? != 0,
                is_complete: row.get::<_, i64>(4)? != 0,
                fetched_count: row.get::<_, i64>(1)? as usize,
                next_cursor: row.get(0)?,
                generation: row.get(2)?,
                last_error: row.get(5)?,
                updated_at: row.get(6)?,
            });
        }
        Ok(CatalogRefreshStatus {
            source_id: source_id.to_string(),
            safety_mode,
            is_running: false,
            is_complete: false,
            fetched_count: 0,
            next_cursor: None,
            generation: 0,
            last_error: None,
            updated_at: None,
        })
    }

    pub fn upsert_skills(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
        generation: i64,
        skills: &[CatalogSkill],
    ) -> AppResult<()> {
        let mut conn = self.connect()?;
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO catalog_skills (
                    source_id, safety_mode, slug, id, name, source_name, source_icon,
                    source_path, relative_path, description, tags_json, supported_agents_json,
                    published_at, updated_at, download_count, install_count, has_skill_md,
                    has_scripts, has_references, has_assets, generation
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
                ON CONFLICT(source_id, safety_mode, slug) DO UPDATE SET
                    id=excluded.id,
                    name=excluded.name,
                    source_name=excluded.source_name,
                    source_icon=excluded.source_icon,
                    source_path=excluded.source_path,
                    relative_path=excluded.relative_path,
                    description=excluded.description,
                    tags_json=excluded.tags_json,
                    supported_agents_json=excluded.supported_agents_json,
                    published_at=excluded.published_at,
                    updated_at=excluded.updated_at,
                    download_count=excluded.download_count,
                    install_count=excluded.install_count,
                    has_skill_md=excluded.has_skill_md,
                    has_scripts=excluded.has_scripts,
                    has_references=excluded.has_references,
                    has_assets=excluded.has_assets,
                    generation=excluded.generation
                "#,
            )?;
            for skill in skills {
                stmt.execute(params![
                    source_id,
                    safety_mode.as_str(),
                    skill.relative_path,
                    skill.id,
                    skill.name,
                    skill.source_name,
                    skill.source_icon,
                    skill.source_path,
                    skill.relative_path,
                    skill.description,
                    serde_json::to_string(&skill.tags)?,
                    serde_json::to_string(&skill.supported_agents)?,
                    skill.published_at,
                    skill.updated_at,
                    skill.download_count.map(|value| value as i64),
                    skill.install_count.map(|value| value as i64),
                    skill.has_skill_md as i64,
                    skill.has_scripts as i64,
                    skill.has_references as i64,
                    skill.has_assets as i64,
                    generation,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn finish_refresh(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
        generation: i64,
        fetched_count: usize,
    ) -> AppResult<()> {
        let conn = self.connect()?;
        conn.execute(
            "DELETE FROM catalog_skills WHERE source_id = ?1 AND safety_mode = ?2 AND generation <> ?3",
            params![source_id, safety_mode.as_str(), generation],
        )?;
        self.save_refresh_state(RefreshStatePatch {
            source_id,
            safety_mode,
            cursor: None,
            fetched_count,
            generation,
            is_running: false,
            is_complete: true,
            last_error: None,
        })
    }

    pub fn count(&self, source_id: &str, safety_mode: CatalogSafetyMode) -> AppResult<usize> {
        let conn = self.connect()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM catalog_skills WHERE source_id = ?1 AND safety_mode = ?2",
            params![source_id, safety_mode.as_str()],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn query(
        &self,
        source: &CatalogSource,
        query: &str,
        sort: CatalogSort,
        filters: &CatalogFilters,
        installed_titles: &HashSet<String>,
        page: usize,
        page_size: usize,
    ) -> AppResult<CatalogSearchResult> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 500);
        let offset = page.saturating_sub(1).saturating_mul(page_size);
        let conn = self.connect()?;
        let mut clauses = vec!["source_id = ?".to_string(), "safety_mode = ?".to_string()];
        let mut values = vec![
            rusqlite::types::Value::Text(source.id.clone()),
            rusqlite::types::Value::Text(filters.safety_mode.as_str().to_string()),
        ];
        let q = query.trim().to_ascii_lowercase();
        if !q.is_empty() {
            clauses.push("(lower(name) LIKE ? OR lower(description) LIKE ? OR lower(relative_path) LIKE ? OR lower(tags_json) LIKE ?)".to_string());
            let pattern = format!("%{}%", q);
            for _ in 0..4 {
                values.push(rusqlite::types::Value::Text(pattern.clone()));
            }
        }
        if let Some(has_data) = filters.has_download_data {
            clauses.push(if has_data {
                "(download_count IS NOT NULL OR install_count IS NOT NULL)".to_string()
            } else {
                "(download_count IS NULL AND install_count IS NULL)".to_string()
            });
        }
        if let Some(days) = filters.time_window_days {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
            clauses.push("(updated_at IS NOT NULL AND updated_at >= ?)".to_string());
            values.push(rusqlite::types::Value::Text(cutoff.to_rfc3339()));
        }
        if !filters.install_statuses.is_empty() {
            let wants_installed = filters
                .install_statuses
                .contains(&CatalogInstallStatus::Installed);
            let wants_not_installed = filters
                .install_statuses
                .contains(&CatalogInstallStatus::NotInstalled);

            match (
                wants_installed,
                wants_not_installed,
                installed_titles.is_empty(),
            ) {
                (true, false, true) => clauses.push("1 = 0".to_string()),
                (true, false, false) => {
                    clauses.push(format!(
                        "lower(name) IN ({})",
                        installed_titles
                            .iter()
                            .map(|_| "?")
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    for title in installed_titles {
                        values.push(rusqlite::types::Value::Text(title.clone()));
                    }
                }
                (false, true, false) => {
                    clauses.push(format!(
                        "lower(name) NOT IN ({})",
                        installed_titles
                            .iter()
                            .map(|_| "?")
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    for title in installed_titles {
                        values.push(rusqlite::types::Value::Text(title.clone()));
                    }
                }
                (false, false, _) => clauses.push("1 = 0".to_string()),
                _ => {}
            }
        }
        if !filters.agent_types.is_empty() {
            clauses.push(format!(
                "({})",
                filters
                    .agent_types
                    .iter()
                    .map(|_| "supported_agents_json LIKE ?")
                    .collect::<Vec<_>>()
                    .join(" OR ")
            ));
            for agent in &filters.agent_types {
                values.push(rusqlite::types::Value::Text(format!("%{}%", agent)));
            }
        }
        if !filters.content_capabilities.is_empty() {
            for capability in &filters.content_capabilities {
                match capability.as_str() {
                    "scripts" => clauses.push("has_scripts = 1".to_string()),
                    "references" => clauses.push("has_references = 1".to_string()),
                    "assets" => clauses.push("has_assets = 1".to_string()),
                    "skillMdOnly" => clauses.push(
                        "(has_skill_md = 1 AND has_scripts = 0 AND has_references = 0 AND has_assets = 0)"
                            .to_string(),
                    ),
                    _ => {}
                }
            }
        }
        let where_sql = clauses.join(" AND ");
        let total_sql = format!("SELECT COUNT(*) FROM catalog_skills WHERE {where_sql}");
        let total: i64 = conn.query_row(
            &total_sql,
            rusqlite::params_from_iter(values.clone()),
            |row| row.get(0),
        )?;
        let order_sql = match sort {
            CatalogSort::Downloads => "COALESCE(download_count, install_count, -1) DESC, name ASC",
            CatalogSort::PublishedDesc => "published_at IS NULL ASC, published_at DESC, name ASC",
            CatalogSort::UpdatedDesc => "updated_at IS NULL ASC, updated_at DESC, name ASC",
            CatalogSort::Source => "source_name ASC, name ASC",
        };
        let query_sql = format!(
            "SELECT id, name, source_id, source_name, source_icon, source_path, relative_path, description, tags_json, supported_agents_json, published_at, updated_at, download_count, install_count, has_skill_md, has_scripts, has_references, has_assets FROM catalog_skills WHERE {where_sql} ORDER BY {order_sql} LIMIT ? OFFSET ?"
        );
        values.push(rusqlite::types::Value::Integer(page_size as i64));
        values.push(rusqlite::types::Value::Integer(offset as i64));
        let mut stmt = conn.prepare(&query_sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values), |row| {
            let tags_json: String = row.get(8)?;
            let supported_agents_json: String = row.get(9)?;
            let name: String = row.get(1)?;
            let install_status = if installed_titles.contains(&name.trim().to_lowercase()) {
                CatalogInstallStatus::Installed
            } else {
                CatalogInstallStatus::NotInstalled
            };
            Ok(CatalogSkill {
                id: row.get(0)?,
                name,
                source_id: row.get(2)?,
                source_name: row.get(3)?,
                source_icon: row.get(4)?,
                source_path: row.get(5)?,
                relative_path: row.get(6)?,
                description: row.get(7)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                supported_agents: serde_json::from_str(&supported_agents_json).unwrap_or_default(),
                published_at: row.get(10)?,
                updated_at: row.get(11)?,
                download_count: row.get::<_, Option<i64>>(12)?.map(|value| value as u64),
                install_count: row.get::<_, Option<i64>>(13)?.map(|value| value as u64),
                has_skill_md: row.get::<_, i64>(14)? != 0,
                has_scripts: row.get::<_, i64>(15)? != 0,
                has_references: row.get::<_, i64>(16)? != 0,
                has_assets: row.get::<_, i64>(17)? != 0,
                install_status,
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(CatalogSearchResult {
            has_more: offset + items.len() < total as usize,
            items,
            total: total as usize,
            page,
            page_size,
        })
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        AppError::Message(format!("SQLite 操作失败: {}", error))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        catalog_index::{CatalogIndex, RefreshStatePatch},
        models::{CatalogFilters, CatalogSafetyMode, CatalogSkill, CatalogSort, CatalogSource},
        service::built_in_catalog_sources_for_test,
    };

    fn clawhub_source() -> CatalogSource {
        built_in_catalog_sources_for_test()
            .into_iter()
            .find(|source| source.id == "clawhub")
            .unwrap()
    }

    fn skill(slug: &str, downloads: u64) -> CatalogSkill {
        let source = clawhub_source();
        CatalogSkill {
            id: format!("clawhub::{slug}"),
            name: format!("Skill {slug}"),
            source_id: source.id.clone(),
            source_name: source.name.clone(),
            source_icon: source.icon.clone(),
            source_path: format!("clawhub://{slug}"),
            relative_path: slug.to_string(),
            description: Some(format!("Description {slug}")),
            tags: vec!["test".to_string()],
            supported_agents: vec!["openclaw".to_string()],
            published_at: Some(format!("2026-01-{downloads:02}T00:00:00Z")),
            updated_at: Some(format!("2026-02-{downloads:02}T00:00:00Z")),
            download_count: Some(downloads),
            install_count: Some(downloads + 10),
            has_skill_md: true,
            has_scripts: false,
            has_references: false,
            has_assets: false,
            install_status: crate::models::CatalogInstallStatus::NotInstalled,
        }
    }

    #[test]
    fn indexes_and_pages_clawhub_skills_without_loading_everything() {
        let dir = tempfile::tempdir().unwrap();
        let index = CatalogIndex::new(dir.path()).unwrap();
        index
            .begin_refresh("clawhub", CatalogSafetyMode::All)
            .unwrap();
        index
            .upsert_skills(
                "clawhub",
                CatalogSafetyMode::All,
                1,
                &(0..250)
                    .map(|i| skill(&format!("skill-{i:03}"), i as u64))
                    .collect::<Vec<_>>(),
            )
            .unwrap();

        let result = index
            .query(
                &clawhub_source(),
                "",
                CatalogSort::Downloads,
                &CatalogFilters::default(),
                &HashSet::new(),
                2,
                100,
            )
            .unwrap();

        assert_eq!(result.total, 250);
        assert_eq!(result.items.len(), 100);
        assert_eq!(result.page, 2);
        assert!(result.has_more);
    }

    #[test]
    fn stores_refresh_cursor_for_resume() {
        let dir = tempfile::tempdir().unwrap();
        let index = CatalogIndex::new(dir.path()).unwrap();
        let generation = index
            .begin_refresh("clawhub", CatalogSafetyMode::All)
            .unwrap();
        index
            .save_refresh_state(RefreshStatePatch {
                source_id: "clawhub",
                safety_mode: CatalogSafetyMode::All,
                cursor: Some("cursor-2"),
                fetched_count: 400,
                generation,
                is_running: false,
                is_complete: false,
                last_error: Some("network"),
            })
            .unwrap();

        let status = index
            .refresh_status("clawhub", CatalogSafetyMode::All)
            .unwrap();

        assert_eq!(status.next_cursor, Some("cursor-2".to_string()));
        assert_eq!(status.fetched_count, 400);
        assert!(!status.is_complete);
        assert_eq!(status.last_error, Some("network".to_string()));
    }

    #[test]
    fn keeps_all_and_non_suspicious_indexes_separate() {
        let dir = tempfile::tempdir().unwrap();
        let index = CatalogIndex::new(dir.path()).unwrap();
        index
            .upsert_skills("clawhub", CatalogSafetyMode::All, 1, &[skill("all", 1)])
            .unwrap();
        index
            .upsert_skills(
                "clawhub",
                CatalogSafetyMode::NonSuspicious,
                2,
                &[skill("safe", 2)],
            )
            .unwrap();

        let mut filters = CatalogFilters::default();
        filters.safety_mode = CatalogSafetyMode::All;
        let all = index
            .query(
                &clawhub_source(),
                "",
                CatalogSort::Source,
                &filters,
                &HashSet::new(),
                1,
                100,
            )
            .unwrap();
        filters.safety_mode = CatalogSafetyMode::NonSuspicious;
        let safe = index
            .query(
                &clawhub_source(),
                "",
                CatalogSort::Source,
                &filters,
                &HashSet::new(),
                1,
                100,
            )
            .unwrap();

        assert_eq!(all.items[0].relative_path, "all");
        assert_eq!(safe.items[0].relative_path, "safe");
    }
}
