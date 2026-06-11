use crate::{
    adapter::{adapter_for, built_in_adapters, AgentAdapter},
    catalog::{
        scan_catalog_repository, scan_clawhub_api_cache, sort_catalog_skills,
        CLAWHUB_API_CACHE_FILE,
    },
    cherry_studio::CherryStudioAdapter,
    error::{AppError, AppResult},
    hash::{copy_dir_all, hash_dir},
    manifest::{read_skill, scan_repository},
    mcp_service::McpService,
    models::{
        AgentProfile, AgentSkillCopy, AgentType, CatalogFilters, CatalogInstallStatus,
        CatalogRefreshResult, CatalogSkill, CatalogSort, CatalogSource, CatalogSourceKind,
        ConflictPolicy, GroupedSkill, ImportSkillFile, ImportSkillResult, InitialData,
        InstallResult,
    },
    store::AppStore,
};
use chrono::{DateTime, Utc};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, SystemTime},
};
use zip::ZipArchive;

pub struct AppService {
    store: AppStore,
    mcp_service: McpService,
}

impl AppService {
    pub fn new() -> AppResult<Self> {
        let store = AppStore::new()?;
        let store_ref = Arc::new(AppStore::new()?);
        Ok(Self {
            store,
            mcp_service: McpService::new(store_ref),
        })
    }

    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let store = AppStore::in_memory()?;
        let store_ref = Arc::new(AppStore::in_memory()?);
        Ok(Self {
            store,
            mcp_service: McpService::new(store_ref),
        })
    }

    pub fn store(&self) -> &AppStore {
        &self.store
    }

    pub fn mcp(&self) -> &McpService {
        &self.mcp_service
    }

    pub fn data_dir(&self) -> PathBuf {
        self.store.data_dir()
    }

    pub fn catalog_cache_root(&self) -> PathBuf {
        self.data_dir().join("catalog-repositories")
    }

    fn catalog_cache_path(&self, source: &CatalogSource) -> PathBuf {
        source
            .cache_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.catalog_cache_root().join(safe_label(&source.id)))
    }

    pub fn backup_root(&self) -> PathBuf {
        self.store.backup_root()
    }

    pub fn import_root(&self) -> PathBuf {
        self.store.import_root()
    }

    pub fn detect_agents(&self) -> AppResult<Vec<AgentProfile>> {
        let mut agents = Vec::new();
        for adapter in built_in_adapters() {
            agents.extend(adapter.detect());
        }
        Ok(agents)
    }

    pub fn get_initial_data(&self) -> AppResult<InitialData> {
        let agents = self.list_agents().unwrap_or_default();
        let skills = self.scan_agent_skills().unwrap_or_default();
        let no_full_coverage_titles = self.store.list_no_full_coverage().unwrap_or_default();
        let no_full_coverage_mcp_titles =
            self.store.list_no_full_coverage_mcp().unwrap_or_default();
        Ok(InitialData {
            skills,
            agents,
            no_full_coverage_titles,
            no_full_coverage_mcp_titles,
        })
    }

    pub fn toggle_no_full_coverage(&self, title: &str) -> AppResult<bool> {
        self.store.toggle_no_full_coverage(title)
    }

    pub fn toggle_no_full_coverage_mcp(&self, title: &str) -> AppResult<bool> {
        self.store.toggle_no_full_coverage_mcp(title)
    }

    pub fn list_saved_agents(&self) -> AppResult<Vec<AgentProfile>> {
        self.store.list_agents()
    }

    pub fn list_agents(&self) -> AppResult<Vec<AgentProfile>> {
        let mut agents = HashMap::new();
        for agent in self.list_saved_agents()? {
            agents.insert(agent.id.clone(), agent);
        }
        for agent in self.detect_agents()? {
            agents.entry(agent.id.clone()).or_insert(agent);
        }
        let mut values = agents.into_values().collect::<Vec<_>>();
        values.sort_by(|a, b| a.name.cmp(&b.name).then(a.skills_path.cmp(&b.skills_path)));
        Ok(values)
    }

    pub fn add_agent(&self, profile: AgentProfile) -> AppResult<AgentProfile> {
        let adapter = adapter_for(&profile);
        adapter.validate(&profile)?;
        self.store.save_agent(&profile)?;
        Ok(profile)
    }

    pub fn remove_agent(&self, agent_id: &str) -> AppResult<()> {
        self.store.remove_agent(agent_id)
    }

    pub fn list_catalog_sources(&self) -> AppResult<Vec<CatalogSource>> {
        let mut sources = built_in_catalog_sources();
        sources.extend(self.store.list_catalog_sources()?);
        for source in &mut sources {
            let cache_path = self.catalog_cache_path(source);
            if source.cache_path.is_none() {
                source.cache_path = Some(cache_path.to_string_lossy().to_string());
            }
            if source.last_refreshed_at.is_none() && cache_path.exists() {
                source.last_refreshed_at = fs::metadata(&cache_path)
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .map(|time| DateTime::<Utc>::from(time).to_rfc3339());
            }
        }
        Ok(sources)
    }

    pub fn save_catalog_source(&self, mut source: CatalogSource) -> AppResult<CatalogSource> {
        if source.id.trim().is_empty() {
            source.id = format!("custom-{}", chrono::Utc::now().timestamp_millis());
        }
        source.kind = CatalogSourceKind::Custom;
        source.icon = if source.icon.trim().is_empty() {
            "custom".to_string()
        } else {
            source.icon
        };
        source.enabled = true;
        source.cache_path = Some(
            self.catalog_cache_root()
                .join(safe_label(&source.id))
                .to_string_lossy()
                .to_string(),
        );
        self.store.save_catalog_source(&source)?;
        Ok(source)
    }

    pub fn refresh_catalog_source(&self, source_id: &str) -> AppResult<CatalogRefreshResult> {
        let mut source = self
            .list_catalog_sources()?
            .into_iter()
            .find(|source| source.id == source_id)
            .ok_or_else(|| AppError::Message(format!("找不到仓库源: {}", source_id)))?;
        let cache_path = self.catalog_cache_path(&source);
        fs::create_dir_all(self.catalog_cache_root())?;

        let skill_count = if source.id == "clawhub" {
            self.refresh_clawhub_api_cache(&cache_path)?
        } else if cache_path.join(".git").is_dir() {
            let output = Command::new("git")
                .arg("-C")
                .arg(&cache_path)
                .arg("pull")
                .arg("--ff-only")
                .output()?;
            if !output.status.success() {
                return Err(AppError::Message(format!(
                    "刷新仓库失败: {}",
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            scan_catalog_repository(&cache_path, &source)?.len()
        } else {
            let output = Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("1")
                .arg(&source.url)
                .arg(&cache_path)
                .output()?;
            if !output.status.success() {
                return Err(AppError::Message(format!(
                    "克隆仓库失败: {}",
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            scan_catalog_repository(&cache_path, &source)?.len()
        };

        source.last_refreshed_at = Some(chrono::Utc::now().to_rfc3339());
        source.cache_path = Some(cache_path.to_string_lossy().to_string());
        if source.kind == CatalogSourceKind::Custom {
            self.store.save_catalog_source(&source)?;
        }
        Ok(CatalogRefreshResult {
            source_id: source.id,
            refreshed: true,
            skill_count,
            message: format!("已刷新 {} 个 catalog skills。", skill_count),
        })
    }

    pub fn search_catalog_skills(
        &self,
        query: Option<&str>,
        sort: CatalogSort,
        filters: CatalogFilters,
    ) -> AppResult<Vec<CatalogSkill>> {
        let sources = self.list_catalog_sources()?;
        let installed = self.scan_agent_skills().unwrap_or_default();
        let installed_titles = installed
            .iter()
            .map(|skill| normalize_title(&skill.title))
            .collect::<HashSet<_>>();
        let q = query.unwrap_or("").trim().to_ascii_lowercase();
        let mut skills = Vec::new();

        for source in sources.into_iter().filter(|source| source.enabled) {
            let cache_path = self.catalog_cache_path(&source);
            if !cache_path.exists() {
                continue;
            }
            let mut source_skills =
                if source.id == "clawhub" && cache_path.join(CLAWHUB_API_CACHE_FILE).exists() {
                    scan_clawhub_api_cache(&cache_path, &source)?
                } else {
                    scan_catalog_repository(&cache_path, &source)?
                };
            for skill in &mut source_skills {
                if installed_titles.contains(&normalize_title(&skill.name)) {
                    skill.install_status = CatalogInstallStatus::Installed;
                }
            }
            skills.extend(source_skills);
        }

        let filtered = skills
            .into_iter()
            .filter(|skill| catalog_matches_query(skill, &q))
            .filter(|skill| catalog_matches_filters(skill, &filters))
            .collect::<Vec<_>>();
        Ok(sort_catalog_skills(filtered, sort))
    }

    pub fn install_catalog_skill(
        &self,
        catalog_skill_id: &str,
        target_agent_ids: Vec<String>,
        conflict_policy: ConflictPolicy,
    ) -> AppResult<Vec<InstallResult>> {
        if target_agent_ids.is_empty() {
            return Err(AppError::Message("请至少选择一个目标 Agent。".to_string()));
        }
        let skill = self
            .search_catalog_skills(None, CatalogSort::UpdatedDesc, CatalogFilters::default())?
            .into_iter()
            .find(|skill| skill.id == catalog_skill_id)
            .ok_or_else(|| {
                AppError::Message(format!("找不到 catalog skill: {}", catalog_skill_id))
            })?;
        let materialized_source;
        let source_path = if skill.source_path.starts_with("clawhub://") {
            materialized_source = self.materialize_clawhub_skill(&skill)?;
            materialized_source.as_path()
        } else {
            Path::new(&skill.source_path)
        };
        let source_dir_name = source_path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::Message("来源 skill 路径无效".to_string()))?;
        let source_fingerprint = hash_dir(source_path).unwrap_or_default();
        let agents = self.list_agents()?;
        let agent_map: HashMap<_, _> = agents
            .into_iter()
            .map(|agent| (agent.id.clone(), agent))
            .collect();
        let mut results = Vec::new();

        for agent_id in target_agent_ids {
            let agent = agent_map
                .get(&agent_id)
                .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;
            fs::create_dir_all(&agent.skills_path)?;
            let mut target = Path::new(&agent.skills_path).join(source_dir_name);
            let mut action = if target.exists() {
                "updated"
            } else {
                "installed"
            }
            .to_string();
            let mut backup_path = None;

            if target.exists() {
                match conflict_policy {
                    ConflictPolicy::Prompt => {
                        return Err(AppError::Message(
                            "目标已存在。请先选择备份覆盖、跳过冲突或另存副本策略。".to_string(),
                        ));
                    }
                    ConflictPolicy::Skip => {
                        results.push(InstallResult {
                            agent_id: agent.id.clone(),
                            skill_id: skill.name.clone(),
                            action: "skipped".to_string(),
                            target_path: target.to_string_lossy().to_string(),
                            backup_path: None,
                            message: format!("已跳过 {}", skill.name),
                        });
                        continue;
                    }
                    ConflictPolicy::Rename => {
                        let suffix = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
                        target = Path::new(&agent.skills_path)
                            .join(format!("{}-{}", source_dir_name, suffix));
                        action = "renamed".to_string();
                    }
                    ConflictPolicy::BackupOverwrite => {
                        let backup = self
                            .backup_root()
                            .join(safe_label(&agent.id))
                            .join(safe_label(&skill.name))
                            .join(chrono::Utc::now().format("%Y%m%d%H%M%S").to_string());
                        copy_dir_all(&target, &backup)?;
                        backup_path = Some(backup.to_string_lossy().to_string());
                    }
                }
            }

            copy_dir_all(source_path, &target)?;
            self.store.record_install(
                &agent.id,
                &skill.name,
                &source_fingerprint,
                &target.to_string_lossy(),
                &action,
                backup_path.as_deref(),
            )?;
            results.push(InstallResult {
                agent_id: agent.id.clone(),
                skill_id: skill.name.clone(),
                action,
                target_path: target.to_string_lossy().to_string(),
                backup_path,
                message: format!("已安装 {} 到 {}", skill.name, agent.name),
            });
        }
        Ok(results)
    }

    pub fn scan_agent_skills(&self) -> AppResult<Vec<GroupedSkill>> {
        let agents = self.list_agents()?;
        let mut copies = Vec::new();
        for agent in &agents {
            copies.extend(scan_agent_skill_copies(agent)?);
        }
        Ok(group_agent_skills(&agents, copies))
    }

    pub fn sync_grouped_skill(
        &self,
        title: &str,
        source_agent_id: Option<&str>,
        target_agent_ids: Vec<String>,
        conflict_policy: ConflictPolicy,
    ) -> AppResult<Vec<InstallResult>> {
        let groups = self.scan_agent_skills()?;
        let group = groups
            .into_iter()
            .find(|group| group.title == title)
            .ok_or_else(|| AppError::Message(format!("找不到 Skill: {}", title)))?;
        let source = match source_agent_id {
            Some(agent_id) => group
                .copies
                .iter()
                .find(|copy| copy.agent_id == agent_id)
                .ok_or_else(|| {
                    AppError::Message(format!("{} 没有可用来源: {}", title, agent_id))
                })?,
            None => &group.best_copy,
        };
        let agents = self.list_agents()?;
        let agent_map: HashMap<_, _> = agents
            .into_iter()
            .map(|agent| (agent.id.clone(), agent))
            .collect();
        let source_path = Path::new(&source.skill_path);
        let source_fingerprint = hash_dir(source_path)?;
        let source_dir_name = source_path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::Message("来源 skill 路径无效".to_string()))?;
        let mut results = Vec::new();

        for agent_id in target_agent_ids {
            if agent_id == source.agent_id {
                results.push(InstallResult {
                    agent_id,
                    skill_id: title.to_string(),
                    action: "skipped".to_string(),
                    target_path: source.skill_path.clone(),
                    backup_path: None,
                    message: format!("{} 已存在于来源 Agent", title),
                });
                continue;
            }
            let agent = agent_map
                .get(&agent_id)
                .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;
            fs::create_dir_all(&agent.skills_path)?;
            let mut target = Path::new(&agent.skills_path).join(source_dir_name);
            let mut action = if target.exists() {
                "updated"
            } else {
                "installed"
            }
            .to_string();
            let mut backup_path = None;

            if target.exists() {
                match conflict_policy {
                    ConflictPolicy::Prompt => {
                        return Err(AppError::Message(
                            "目标已存在。请先选择备份覆盖、跳过冲突或另存副本策略。".to_string(),
                        ));
                    }
                    ConflictPolicy::Skip => {
                        results.push(InstallResult {
                            agent_id: agent.id.clone(),
                            skill_id: title.to_string(),
                            action: "skipped".to_string(),
                            target_path: target.to_string_lossy().to_string(),
                            backup_path: None,
                            message: format!("已跳过 {}", title),
                        });
                        continue;
                    }
                    ConflictPolicy::Rename => {
                        let suffix = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
                        target = Path::new(&agent.skills_path)
                            .join(format!("{}-{}", source_dir_name, suffix));
                        action = "renamed".to_string();
                    }
                    ConflictPolicy::BackupOverwrite => {
                        let backup = self
                            .backup_root()
                            .join(safe_label(&agent.id))
                            .join(safe_label(title))
                            .join(chrono::Utc::now().format("%Y%m%d%H%M%S").to_string());
                        copy_dir_all(&target, &backup)?;
                        fs::remove_dir_all(&target)?;
                        backup_path = Some(backup.to_string_lossy().to_string());
                    }
                }
            }

            if agent.agent_type == AgentType::CherryStudio {
                if let Some(cs) = CherryStudioAdapter::new() {
                    cs.install_skill(source_path, source_dir_name)?;
                }
            } else {
                copy_dir_all(source_path, &target)?;
            }
            self.store.record_install(
                &agent.id,
                title,
                &source_fingerprint,
                &target.to_string_lossy(),
                &action,
                backup_path.as_deref(),
            )?;
            results.push(InstallResult {
                agent_id: agent.id.clone(),
                skill_id: title.to_string(),
                action,
                target_path: target.to_string_lossy().to_string(),
                backup_path,
                message: format!("{} 已同步到 {}", title, agent.name),
            });
        }
        Ok(results)
    }

    pub fn uninstall_skill(&self, skill_id: &str, agent_id: &str) -> AppResult<()> {
        let agent = self
            .list_agents()?
            .into_iter()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;

        if agent.agent_type == AgentType::CherryStudio {
            if let Some(cs) = CherryStudioAdapter::new() {
                cs.uninstall_skill(skill_id)?;
            }
        } else {
            let adapter = adapter_for(&agent);
            adapter.uninstall(skill_id, &agent, &self.backup_root())?;
        }

        let target_path = Path::new(&agent.skills_path)
            .join(skill_id)
            .to_string_lossy()
            .to_string();
        self.store
            .record_uninstall(agent_id, skill_id, &target_path, None)
    }

    pub fn uninstall_skill_from_agents(
        &self,
        skill_id: &str,
        agent_ids: &[String],
    ) -> AppResult<()> {
        for agent_id in agent_ids {
            self.uninstall_skill(skill_id, agent_id)?;
        }
        Ok(())
    }

    pub fn rollback_last(&self, agent_id: &str, skill_id: &str) -> AppResult<()> {
        let (target, backup) = self
            .store
            .last_backup(agent_id, skill_id)?
            .ok_or_else(|| AppError::Message("没有可回滚的备份".to_string()))?;
        let target_path = Path::new(&target);
        if target_path.exists() {
            return Err(AppError::Message(
                "目标目录已存在。为避免批量删除，请先手动处理目标目录后再回滚。".to_string(),
            ));
        }
        copy_dir_all(Path::new(&backup), target_path)?;
        Ok(())
    }

    pub fn import_uploaded_files(
        &self,
        file_name: &str,
        files: &[ImportSkillFile],
        target_agent_ids: &[String],
        conflict_policy: ConflictPolicy,
    ) -> AppResult<ImportSkillResult> {
        if files.is_empty() {
            return Err(AppError::Message("上传内容为空".to_string()));
        }
        if target_agent_ids.is_empty() {
            return Err(AppError::Message("请至少选择一个目标 Agent。".to_string()));
        }

        let source_root = if files.len() == 1 && file_name.to_ascii_lowercase().ends_with(".zip") {
            self.unpack_zip_bytes(&files[0].bytes, file_name)?
        } else {
            self.write_uploaded_files(files)?
        };

        self.import_from_source_dir(&source_root, target_agent_ids, conflict_policy)
    }

    fn import_from_source_dir(
        &self,
        source_root: &Path,
        target_agent_ids: &[String],
        conflict_policy: ConflictPolicy,
    ) -> AppResult<ImportSkillResult> {
        let dirs = self.manifest_source_dirs(source_root)?;
        if dirs.is_empty() {
            // Provide a more descriptive error with directory contents hint
            let mut hint = String::new();
            if let Ok(entries) = fs::read_dir(source_root) {
                let names: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            format!("{}/", name)
                        } else {
                            name
                        }
                    })
                    .collect();
                if !names.is_empty() {
                    let preview = if names.len() > 5 {
                        format!("{}... 等 {} 项", names[..5].join(", "), names.len())
                    } else {
                        names.join(", ")
                    };
                    hint = format!("，目录内容：[{}]", preview);
                }
            }
            return Err(AppError::Message(format!(
                "没有发现可识别的 skill manifest（需要 skill.json、skill.yaml 或 skill.yml）{}。",
                hint
            )));
        }

        let agents = self.list_agents()?;
        let agent_map: HashMap<_, _> = agents.into_iter().map(|a| (a.id.clone(), a)).collect();

        let mut imported = 0;
        let mut skipped = 0;

        for source in &dirs {
            let skill = read_skill(&self.manifest_path_for(source)?)?;
            let skill_dir_name = source
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or_else(|| AppError::Message("skill 目录名无效".to_string()))?;

            for agent_id in target_agent_ids {
                let agent = agent_map
                    .get(agent_id)
                    .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;
                fs::create_dir_all(&agent.skills_path)?;
                let mut target = Path::new(&agent.skills_path).join(skill_dir_name);

                if target.exists() {
                    match conflict_policy {
                        ConflictPolicy::Prompt => {
                            return Err(AppError::Message(
                                "目标已存在。请先选择备份覆盖、跳过冲突或另存副本策略。"
                                    .to_string(),
                            ));
                        }
                        ConflictPolicy::Skip => {
                            skipped += 1;
                            continue;
                        }
                        ConflictPolicy::Rename => {
                            let suffix = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
                            target = Path::new(&agent.skills_path)
                                .join(format!("{}-{}", skill_dir_name, suffix));
                        }
                        ConflictPolicy::BackupOverwrite => {
                            let backup = self
                                .backup_root()
                                .join(safe_label(&agent.id))
                                .join(safe_label(&skill.manifest.id))
                                .join(chrono::Utc::now().format("%Y%m%d%H%M%S").to_string());
                            copy_dir_all(&target, &backup)?;
                            fs::remove_dir_all(&target)?;
                        }
                    }
                }

                if agent.agent_type == AgentType::CherryStudio {
                    if let Some(cs) = CherryStudioAdapter::new() {
                        cs.install_skill(source, skill_dir_name)?;
                    }
                } else {
                    copy_dir_all(source, &target)?;
                }
                imported += 1;
            }
        }

        Ok(ImportSkillResult {
            imported,
            skipped,
            message: format!(
                "已导入 {} 个 skills，跳过 {} 个已存在 skills。",
                imported, skipped
            ),
        })
    }

    fn manifest_source_dirs(&self, root: &Path) -> AppResult<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        let mut seen = HashSet::new();
        for skill in scan_repository(root)? {
            let source = PathBuf::from(skill.source_path);
            if seen.insert(source.clone()) {
                dirs.push(source);
            }
        }
        Ok(dirs)
    }

    fn manifest_path_for(&self, source: &Path) -> AppResult<PathBuf> {
        ["skill.json", "skill.yaml", "skill.yml"]
            .into_iter()
            .map(|name| source.join(name))
            .find(|path| path.exists())
            .ok_or_else(|| AppError::Message("导入过程中 manifest 不见了。".to_string()))
    }

    fn import_workspace(&self, label: &str) -> AppResult<PathBuf> {
        let workspace = self.import_root().join(format!(
            "{}-{}",
            chrono::Utc::now().timestamp_millis(),
            safe_label(label)
        ));
        fs::create_dir_all(&workspace)?;
        Ok(workspace)
    }

    fn write_uploaded_files(&self, files: &[ImportSkillFile]) -> AppResult<PathBuf> {
        let workspace = self.import_workspace("folder")?;
        for file in files {
            let destination = workspace.join(safe_relative_path(&file.relative_path)?);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(destination, &file.bytes)?;
        }
        Ok(workspace)
    }

    fn refresh_clawhub_api_cache(&self, cache_path: &Path) -> AppResult<usize> {
        fs::create_dir_all(cache_path)?;
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(45))
            .build();
        let mut cursor: Option<String> = None;
        let mut items = Vec::new();

        for _ in 0..20 {
            let mut request = agent
                .get("https://clawhub.ai/api/v1/skills")
                .query("limit", "100");
            if let Some(cursor_value) = cursor.as_deref() {
                request = request.query("cursor", cursor_value);
            }
            let response = request.call().map_err(clawhub_http_error)?;
            let text = response.into_string().map_err(|error| {
                AppError::Message(format!("读取 ClawHub API 响应失败: {}", error))
            })?;
            let value = serde_json::from_str::<serde_json::Value>(&text)?;
            if let Some(page_items) = value.get("items").and_then(|value| value.as_array()) {
                items.extend(page_items.iter().cloned());
            }
            cursor = value
                .get("nextCursor")
                .and_then(|value| value.as_str())
                .map(ToString::to_string);
            if cursor.is_none() {
                break;
            }
        }

        let cache = serde_json::json!({
            "items": items,
            "fetchedAt": chrono::Utc::now().to_rfc3339(),
        });
        fs::write(
            cache_path.join(CLAWHUB_API_CACHE_FILE),
            serde_json::to_vec_pretty(&cache)?,
        )?;
        Ok(cache["items"].as_array().map_or(0, Vec::len))
    }

    fn materialize_clawhub_skill(&self, skill: &CatalogSkill) -> AppResult<PathBuf> {
        let slug = skill
            .source_path
            .strip_prefix("clawhub://")
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::Message("ClawHub skill 来源无效。".to_string()))?;
        let cache_path = self
            .catalog_cache_root()
            .join("clawhub")
            .join("downloaded")
            .join(safe_label(slug));
        if cache_path.join("SKILL.md").exists()
            || cache_path.join("skill.json").exists()
            || cache_path.join("skill.yaml").exists()
            || cache_path.join("skill.yml").exists()
        {
            return Ok(cache_path);
        }

        let mut reader = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(60))
            .build()
            .get("https://clawhub.ai/api/v1/download")
            .query("slug", slug)
            .call()
            .map_err(clawhub_http_error)?
            .into_reader();
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|error| AppError::Message(format!("下载 ClawHub skill 失败: {}", error)))?;
        let extracted = self.unpack_zip_bytes(&bytes, slug)?;
        let source = CatalogSource {
            id: "clawhub".to_string(),
            name: "ClawHub".to_string(),
            url: "https://clawhub.ai/api/v1/skills".to_string(),
            kind: CatalogSourceKind::BuiltIn,
            icon: "clawhub".to_string(),
            enabled: true,
            last_refreshed_at: None,
            cache_path: None,
        };
        let mut extracted_skills = scan_catalog_repository(&extracted, &source)?;
        let Some(extracted_skill) = extracted_skills.pop() else {
            return Err(AppError::Message(format!(
                "ClawHub skill {} 的下载包中没有找到 SKILL.md。",
                slug
            )));
        };
        copy_dir_all(Path::new(&extracted_skill.source_path), &cache_path)?;
        Ok(cache_path)
    }

    fn unpack_zip_bytes(&self, bytes: &[u8], label: &str) -> AppResult<PathBuf> {
        let workspace = self.import_workspace(label)?;
        let extracted = workspace.join("expanded");
        fs::create_dir_all(&extracted)?;
        let mut archive = ZipArchive::new(Cursor::new(bytes))?;

        let mut extracted_count = 0u32;
        let mut skipped_count = 0u32;

        for index in 0..archive.len() {
            let mut file = archive.by_index(index)?;
            if file.is_dir() {
                continue;
            }

            // Try enclosed_name first (safest), fall back to sanitized raw name
            let file_path = match file.enclosed_name().map(PathBuf::from) {
                Some(path) => path,
                None => {
                    // Fall back to raw name with manual sanitization
                    let raw_name = file.name().replace('\\', "/");
                    let sanitized = sanitize_zip_path(&raw_name);
                    match sanitized {
                        Some(path) => path,
                        None => {
                            skipped_count += 1;
                            continue;
                        }
                    }
                }
            };

            // Skip empty paths
            if file_path.as_os_str().is_empty() {
                skipped_count += 1;
                continue;
            }

            let destination = extracted.join(&file_path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;
            fs::write(destination, contents)?;
            extracted_count += 1;
        }

        if extracted_count == 0 {
            let detail = if skipped_count > 0 {
                format!("（{} 个文件因路径不安全被跳过）", skipped_count)
            } else {
                String::new()
            };
            return Err(AppError::Message(format!(
                "zip 文件中没有可提取的文件{}。",
                detail
            )));
        }

        Ok(extracted)
    }
}

pub fn safe_relative_path(relative_path: &str) -> AppResult<PathBuf> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(AppError::Message(format!(
            "路径必须是相对路径: {}",
            relative_path
        )));
    }

    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => safe.push(value),
            _ => return Err(AppError::Message(format!("路径不安全: {}", relative_path))),
        }
    }

    if safe.as_os_str().is_empty() {
        return Err(AppError::Message("路径不能为空".to_string()));
    }
    Ok(safe)
}

fn safe_label(label: &str) -> String {
    let value = label
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    if value.is_empty() {
        "import".to_string()
    } else {
        value
    }
}

fn clawhub_http_error(error: ureq::Error) -> AppError {
    match error {
        ureq::Error::Status(code, response) => AppError::Message(format!(
            "ClawHub API 请求失败: HTTP {} {}",
            code,
            response.status_text()
        )),
        ureq::Error::Transport(error) => {
            AppError::Message(format!("ClawHub API 请求失败: {}", error))
        }
    }
}

fn built_in_catalog_sources() -> Vec<CatalogSource> {
    vec![
        CatalogSource {
            id: "clawhub".to_string(),
            name: "ClawHub".to_string(),
            url: "https://clawhub.ai/api/v1/skills".to_string(),
            kind: CatalogSourceKind::BuiltIn,
            icon: "clawhub".to_string(),
            enabled: true,
            last_refreshed_at: None,
            cache_path: None,
        },
        CatalogSource {
            id: "claude".to_string(),
            name: "Claude".to_string(),
            url: "https://github.com/anthropics/skills".to_string(),
            kind: CatalogSourceKind::BuiltIn,
            icon: "claude".to_string(),
            enabled: true,
            last_refreshed_at: None,
            cache_path: None,
        },
        CatalogSource {
            id: "codex".to_string(),
            name: "Codex".to_string(),
            url: "https://github.com/openai/skills".to_string(),
            kind: CatalogSourceKind::BuiltIn,
            icon: "codex".to_string(),
            enabled: true,
            last_refreshed_at: None,
            cache_path: None,
        },
    ]
}

fn catalog_matches_query(skill: &CatalogSkill, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let fields = [
        skill.name.as_str(),
        skill.description.as_deref().unwrap_or(""),
        skill.source_name.as_str(),
        skill.relative_path.as_str(),
    ];
    fields
        .iter()
        .any(|field| field.to_ascii_lowercase().contains(query))
        || skill
            .tags
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(query))
}

fn catalog_matches_filters(skill: &CatalogSkill, filters: &CatalogFilters) -> bool {
    if !filters.source_ids.is_empty() && !filters.source_ids.contains(&skill.source_id) {
        return false;
    }
    if !filters.agent_types.is_empty()
        && !filters.agent_types.iter().any(|agent| {
            skill
                .supported_agents
                .iter()
                .any(|supported| supported.eq_ignore_ascii_case(agent))
        })
    {
        return false;
    }
    if !filters.install_statuses.is_empty()
        && !filters.install_statuses.contains(&skill.install_status)
    {
        return false;
    }
    if let Some(has_data) = filters.has_download_data {
        let skill_has_data = skill.download_count.is_some() || skill.install_count.is_some();
        if skill_has_data != has_data {
            return false;
        }
    }
    if !filters.content_capabilities.is_empty() {
        for capability in &filters.content_capabilities {
            let matches = match capability.as_str() {
                "scripts" => skill.has_scripts,
                "references" => skill.has_references,
                "assets" => skill.has_assets,
                "skillMdOnly" => {
                    skill.has_skill_md
                        && !skill.has_scripts
                        && !skill.has_references
                        && !skill.has_assets
                }
                _ => true,
            };
            if !matches {
                return false;
            }
        }
    }
    if let Some(days) = filters.time_window_days {
        let Some(updated_at) = &skill.updated_at else {
            return false;
        };
        let Ok(parsed) = DateTime::parse_from_rfc3339(updated_at) else {
            return false;
        };
        let cutoff = Utc::now() - chrono::Duration::days(days);
        if parsed.with_timezone(&Utc) < cutoff {
            return false;
        }
    }
    true
}

/// Sanitize a raw zip entry path for safe extraction.
/// Returns None if the path is unsafe (absolute, contains traversal, or empty).
fn sanitize_zip_path(raw: &str) -> Option<PathBuf> {
    let path = Path::new(raw);
    let mut safe = PathBuf::new();
    let mut depth = 0i32;

    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let s = name.to_string_lossy();
                // Reject null bytes
                if s.contains('\0') {
                    return None;
                }
                safe.push(name);
                depth += 1;
            }
            Component::ParentDir => {
                // Allow ../ only if we have depth to spare
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                safe.pop();
            }
            Component::CurDir => {
                // Skip ./ components
            }
            _ => {
                // Reject absolute paths, drive letters, etc.
                return None;
            }
        }
    }

    if safe.as_os_str().is_empty() {
        return None;
    }
    Some(safe)
}

fn scan_agent_skill_copies(agent: &AgentProfile) -> AppResult<Vec<AgentSkillCopy>> {
    let root = Path::new(&agent.skills_path);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut copies = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name();
        if dir_name.to_string_lossy().starts_with('.') {
            continue;
        }
        let metadata = fs::metadata(&path).ok();
        let (title, version, description, readme) = read_agent_skill_info(&path);
        copies.push(AgentSkillCopy {
            agent_id: agent.id.clone(),
            agent_name: agent.name.clone(),
            skill_path: path.to_string_lossy().to_string(),
            title,
            version,
            fingerprint: String::new(),
            updated_at: metadata
                .and_then(|metadata| metadata.modified().ok())
                .map(system_time_to_rfc3339),
            description,
            readme,
        });
    }
    Ok(copies)
}

fn group_agent_skills(agents: &[AgentProfile], copies: Vec<AgentSkillCopy>) -> Vec<GroupedSkill> {
    let mut grouped: HashMap<String, Vec<AgentSkillCopy>> = HashMap::new();
    for copy in copies {
        grouped
            .entry(normalize_title(&copy.title))
            .or_default()
            .push(copy);
    }

    let mut values = grouped
        .into_values()
        .map(|mut copies| {
            copies.sort_by(compare_skill_copy);
            let best_copy = copies[0].clone();
            let installed_set = copies
                .iter()
                .map(|copy| copy.agent_id.clone())
                .collect::<HashSet<_>>();
            let missing_agent_ids = agents
                .iter()
                .filter(|agent| !installed_set.contains(&agent.id))
                .map(|agent| agent.id.clone())
                .collect::<Vec<_>>();
            let mut installed_agent_ids = copies
                .iter()
                .map(|copy| copy.agent_id.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            installed_agent_ids.sort();
            GroupedSkill {
                title: best_copy.title.clone(),
                description: best_copy.description.clone(),
                readme: best_copy.readme.clone(),
                best_copy,
                copies,
                installed_agent_ids,
                missing_agent_ids,
            }
        })
        .collect::<Vec<_>>();
    values.sort_by(|a, b| a.title.cmp(&b.title));
    values
}

/// Check if a description contains meaningful text (not just symbols/punctuation).
fn is_valid_description(desc: &str) -> bool {
    desc.chars().any(|c| {
        c.is_alphanumeric() || c.is_ascii_alphabetic() || ('\u{4e00}'..='\u{9fff}').contains(&c)
    })
}

fn read_agent_skill_info(
    skill_path: &Path,
) -> (String, Option<String>, Option<String>, Option<String>) {
    for name in ["skill.json", "skill.yaml", "skill.yml"] {
        let manifest_path = skill_path.join(name);
        if !manifest_path.exists() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&manifest_path) {
            let parsed = match manifest_path.extension().and_then(|value| value.to_str()) {
                Some("json") => serde_json::from_str::<serde_json::Value>(&text).ok(),
                _ => serde_yaml::from_str::<serde_json::Value>(&text).ok(),
            };
            if let Some(value) = parsed {
                let title = value
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                let version = value
                    .get("version")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string);
                let description = value
                    .get("description")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty() && is_valid_description(value))
                    .map(ToString::to_string);
                if let Some(title) = title {
                    let readme = fs::read_to_string(skill_path.join("SKILL.md"))
                        .ok()
                        .and_then(|text| extract_markdown_body(&text));
                    return (title.to_string(), version, description, readme);
                }
            }
        }
    }

    let skill_md = skill_path.join("SKILL.md");
    if let Ok(text) = fs::read_to_string(&skill_md) {
        let readme = extract_markdown_body(&text);
        if let Some((title, version, description)) = read_markdown_frontmatter(&text) {
            return (title, version, description, readme);
        }
        if let Some(title) = read_markdown_heading(&text) {
            return (title, None, None, readme);
        }
    }

    (
        skill_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Untitled Skill")
            .to_string(),
        None,
        None,
        None,
    )
}

fn extract_markdown_body(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.starts_with("---") {
        return Some(trimmed.to_string());
    }
    let after_first = &trimmed[3..];
    let Some(end_idx) = after_first.find("\n---") else {
        return Some(trimmed.to_string());
    };
    let body = after_first[end_idx + 4..].trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
}

fn read_markdown_frontmatter(text: &str) -> Option<(String, Option<String>, Option<String>)> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut title = None;
    let mut version = None;
    let mut description = None;
    let mut collecting_block: Option<String> = None;
    let mut block_lines: Vec<String> = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        // If collecting a block scalar, gather indented continuation lines
        if let Some(ref key) = collecting_block {
            if line.starts_with(' ') || line.starts_with('\t') {
                block_lines.push(trimmed.to_string());
                continue;
            } else {
                // Block scalar ended; store collected value
                let block_value = block_lines.join("\n");
                if !block_value.is_empty() {
                    match key.as_str() {
                        "title" | "name" => title = Some(block_value.clone()),
                        "version" => version = Some(block_value.clone()),
                        "description" => description = Some(block_value.clone()),
                        _ => {}
                    }
                }
                collecting_block = None;
                block_lines.clear();
            }
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        // Detect YAML block scalar indicators (| or >)
        if value == "|"
            || value == ">"
            || value == "|-"
            || value == ">-"
            || value == "|+"
            || value == ">+"
        {
            collecting_block = Some(key.to_string());
            block_lines.clear();
            continue;
        }
        let value = value.trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            continue;
        }
        match key {
            "title" | "name" => title = Some(value.to_string()),
            "version" => version = Some(value.to_string()),
            "description" => {
                if is_valid_description(value) {
                    description = Some(value.to_string());
                }
            }
            _ => {}
        }
    }
    // Handle block scalar that extends to the end of frontmatter
    if let Some(ref key) = collecting_block {
        let block_value = block_lines.join("\n");
        if !block_value.is_empty() {
            match key.as_str() {
                "title" | "name" => title = Some(block_value),
                "version" => version = Some(block_value),
                "description" => {
                    if is_valid_description(&block_value) {
                        description = Some(block_value);
                    }
                }
                _ => {}
            }
        }
    }
    title.map(|title| (title, version, description))
}

fn read_markdown_heading(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("# "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn compare_skill_copy(a: &AgentSkillCopy, b: &AgentSkillCopy) -> Ordering {
    compare_versions(b.version.as_deref(), a.version.as_deref())
        .then_with(|| b.updated_at.cmp(&a.updated_at))
        .then_with(|| a.agent_name.cmp(&b.agent_name))
        .then_with(|| a.skill_path.cmp(&b.skill_path))
}

fn compare_versions(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (parse_version(a), parse_version(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn parse_version(version: Option<&str>) -> Option<Vec<u64>> {
    let version = version?.trim().trim_start_matches('v');
    if version.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for part in version.split('.') {
        let digits = part
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if digits.is_empty() {
            return None;
        }
        parts.push(digits.parse().ok()?);
    }
    Some(parts)
}

fn normalize_title(title: &str) -> String {
    title.trim().to_lowercase()
}

fn system_time_to_rfc3339(time: SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SkillManifest;
    use std::io::Write;

    fn test_service_with_agent(agent_dir: &Path) -> AppService {
        let service = AppService::in_memory().unwrap();
        let profile = AgentProfile {
            id: "test-agent".into(),
            name: "Test Agent".into(),
            agent_type: crate::models::AgentType::Custom,
            skills_path: agent_dir.to_string_lossy().to_string(),
            adapter_config: None,
        };
        service.add_agent(profile).unwrap();
        service
    }

    fn write_demo_skill(root: &Path, id: &str) {
        let skill_dir = root.join(id);
        fs::create_dir_all(&skill_dir).unwrap();
        let manifest = SkillManifest {
            id: id.to_string(),
            name: format!("Skill {}", id),
            version: "1.0.0".to_string(),
            description: Some("demo".to_string()),
            tags: vec![],
            supported_agents: vec!["*".to_string()],
            entry: None,
            files: vec!["SKILL.md".to_string()],
        };
        fs::write(
            skill_dir.join("skill.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(skill_dir.join("SKILL.md"), "hello").unwrap();
    }

    fn collect_upload_files(root: &Path) -> Vec<ImportSkillFile> {
        let mut files = Vec::new();
        collect_files_recursive(root, root, &mut files);
        files
    }

    fn collect_files_recursive(base: &Path, dir: &Path, out: &mut Vec<ImportSkillFile>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                collect_files_recursive(base, &path, out);
            } else {
                let relative = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                out.push(ImportSkillFile {
                    relative_path: relative,
                    bytes: fs::read(&path).unwrap(),
                });
            }
        }
    }

    fn write_agent_skill(
        root: &Path,
        dir: &str,
        manifest_name: Option<&str>,
        version: Option<&str>,
        markdown: &str,
    ) {
        let skill_dir = root.join(dir);
        fs::create_dir_all(&skill_dir).unwrap();
        if manifest_name.is_some() || version.is_some() {
            let manifest = serde_json::json!({
                "id": dir,
                "name": manifest_name.unwrap_or(dir),
                "version": version.unwrap_or("1.0.0"),
                "supportedAgents": ["*"],
                "files": ["SKILL.md"]
            });
            fs::write(
                skill_dir.join("skill.json"),
                serde_json::to_string(&manifest).unwrap(),
            )
            .unwrap();
        }
        fs::write(skill_dir.join("SKILL.md"), markdown).unwrap();
    }

    #[test]
    fn imports_folder_skill() {
        let agent_dir = tempfile::tempdir().unwrap();
        let service = test_service_with_agent(agent_dir.path());
        let upload = tempfile::tempdir().unwrap();
        write_demo_skill(upload.path(), "demo");

        let files = collect_upload_files(upload.path());
        let result = service
            .import_uploaded_files("demo", &files, &["test-agent".into()], ConflictPolicy::Skip)
            .unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 0);
        assert!(agent_dir.path().join("demo").join("skill.json").exists());
    }

    #[test]
    fn skips_duplicate_skill() {
        let agent_dir = tempfile::tempdir().unwrap();
        let service = test_service_with_agent(agent_dir.path());
        let upload = tempfile::tempdir().unwrap();
        write_demo_skill(upload.path(), "demo");

        let files = collect_upload_files(upload.path());
        service
            .import_uploaded_files("demo", &files, &["test-agent".into()], ConflictPolicy::Skip)
            .unwrap();
        let result = service
            .import_uploaded_files("demo", &files, &["test-agent".into()], ConflictPolicy::Skip)
            .unwrap();
        assert_eq!(result.imported, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn reports_empty_upload() {
        let agent_dir = tempfile::tempdir().unwrap();
        let service = test_service_with_agent(agent_dir.path());

        let result = service.import_uploaded_files(
            "empty",
            &[],
            &["test-agent".into()],
            ConflictPolicy::Skip,
        );
        assert!(result.is_err());
    }

    #[test]
    fn imports_zip_skill() {
        let agent_dir = tempfile::tempdir().unwrap();
        let service = test_service_with_agent(agent_dir.path());
        let upload = tempfile::tempdir().unwrap();
        write_demo_skill(upload.path(), "demo");

        let zip_path = upload.path().join("demo.zip");
        {
            let file = fs::File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("demo/skill.json", options).unwrap();
            zip.write_all(&fs::read(upload.path().join("demo").join("skill.json")).unwrap())
                .unwrap();
            zip.start_file("demo/SKILL.md", options).unwrap();
            zip.write_all(b"hello").unwrap();
            zip.finish().unwrap();
        }

        let zip_bytes = fs::read(&zip_path).unwrap();
        let files = vec![ImportSkillFile {
            relative_path: "demo.zip".to_string(),
            bytes: zip_bytes,
        }];
        let result = service
            .import_uploaded_files(
                "demo.zip",
                &files,
                &["test-agent".into()],
                ConflictPolicy::Skip,
            )
            .unwrap();
        assert_eq!(result.imported, 1);
        assert!(agent_dir.path().join("demo").join("skill.json").exists());
    }

    #[test]
    fn reads_agent_skill_title_by_manifest_frontmatter_heading_then_dir() {
        let root = tempfile::tempdir().unwrap();
        write_agent_skill(
            root.path(),
            "manifest",
            Some("Manifest Title"),
            Some("2.0.0"),
            "# Ignored",
        );
        fs::create_dir_all(root.path().join("frontmatter")).unwrap();
        fs::write(
            root.path().join("frontmatter").join("SKILL.md"),
            "---\ntitle: Frontmatter Title\nversion: 1.2.3\n---\n# Ignored",
        )
        .unwrap();
        fs::create_dir_all(root.path().join("heading")).unwrap();
        fs::write(
            root.path().join("heading").join("SKILL.md"),
            "# Heading Title",
        )
        .unwrap();
        fs::create_dir_all(root.path().join("directory")).unwrap();

        assert_eq!(
            read_agent_skill_info(&root.path().join("manifest")),
            (
                "Manifest Title".to_string(),
                Some("2.0.0".to_string()),
                None,
                Some("# Ignored".to_string())
            )
        );
        assert_eq!(
            read_agent_skill_info(&root.path().join("frontmatter")),
            (
                "Frontmatter Title".to_string(),
                Some("1.2.3".to_string()),
                None,
                Some("# Ignored".to_string())
            )
        );
        assert_eq!(
            read_agent_skill_info(&root.path().join("heading")),
            (
                "Heading Title".to_string(),
                None,
                None,
                Some("# Heading Title".to_string())
            )
        );
        assert_eq!(
            read_agent_skill_info(&root.path().join("directory")),
            ("directory".to_string(), None, None, None)
        );
    }

    #[test]
    fn reads_yaml_block_scalar_description() {
        let root = tempfile::tempdir().unwrap();

        // Test block scalar with |
        fs::create_dir_all(root.path().join("block-pipe")).unwrap();
        fs::write(
            root.path().join("block-pipe").join("SKILL.md"),
            "---\nname: humanizer\nversion: 2.1.1\ndescription: |\n  去除文本中的 AI 写作痕迹。\n  适用于润色、审阅。\n---\n# Body",
        )
        .unwrap();

        let (title, version, description, readme) =
            read_agent_skill_info(&root.path().join("block-pipe"));
        assert_eq!(title, "humanizer");
        assert_eq!(version, Some("2.1.1".to_string()));
        assert_eq!(
            description,
            Some("去除文本中的 AI 写作痕迹。\n适用于润色、审阅。".to_string())
        );
        assert_eq!(readme, Some("# Body".to_string()));

        // Test block scalar with >
        fs::create_dir_all(root.path().join("block-gt")).unwrap();
        fs::write(
            root.path().join("block-gt").join("SKILL.md"),
            "---\nname: test-skill\ndescription: >\n  This is a\n  folded description.\nversion: 1.0.0\n---\n# Content",
        )
        .unwrap();

        let (title, version, description, _) = read_agent_skill_info(&root.path().join("block-gt"));
        assert_eq!(title, "test-skill");
        assert_eq!(version, Some("1.0.0".to_string()));
        assert_eq!(
            description,
            Some("This is a\nfolded description.".to_string())
        );

        // Test block scalar extending to end of frontmatter
        fs::create_dir_all(root.path().join("block-eof")).unwrap();
        fs::write(
            root.path().join("block-eof").join("SKILL.md"),
            "---\nname: end-skill\ndescription: |\n  Line one.\n  Line two.\n---",
        )
        .unwrap();

        let (title, _, description, _) = read_agent_skill_info(&root.path().join("block-eof"));
        assert_eq!(title, "end-skill");
        assert_eq!(description, Some("Line one.\nLine two.".to_string()));
    }

    #[test]
    fn groups_agent_skills_and_picks_highest_version() {
        let agent_a = AgentProfile {
            id: "a".into(),
            name: "Agent A".into(),
            agent_type: crate::models::AgentType::Custom,
            skills_path: "a".into(),
            adapter_config: None,
        };
        let agent_b = AgentProfile {
            id: "b".into(),
            name: "Agent B".into(),
            agent_type: crate::models::AgentType::Custom,
            skills_path: "b".into(),
            adapter_config: None,
        };
        let copies = vec![
            AgentSkillCopy {
                agent_id: "a".into(),
                agent_name: "Agent A".into(),
                skill_path: "a/demo".into(),
                title: "Demo".into(),
                version: Some("1.0.0".into()),
                fingerprint: "a".into(),
                updated_at: Some("2026-05-01T00:00:00Z".into()),
                description: None,
                readme: None,
            },
            AgentSkillCopy {
                agent_id: "b".into(),
                agent_name: "Agent B".into(),
                skill_path: "b/demo".into(),
                title: "demo".into(),
                version: Some("2.0.0".into()),
                fingerprint: "b".into(),
                updated_at: Some("2026-05-02T00:00:00Z".into()),
                description: None,
                readme: None,
            },
        ];

        let groups = group_agent_skills(&[agent_a, agent_b], copies);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].best_copy.agent_id, "b");
        assert!(groups[0].missing_agent_ids.is_empty());
    }

    #[test]
    fn syncs_grouped_skill_from_best_agent_copy() {
        let service = AppService::in_memory().unwrap();
        let agent_a_root = tempfile::tempdir().unwrap();
        let agent_b_root = tempfile::tempdir().unwrap();
        write_agent_skill(
            agent_a_root.path(),
            "demo",
            Some("Demo Skill"),
            Some("1.0.0"),
            "# Demo Skill\nold",
        );
        write_agent_skill(
            agent_b_root.path(),
            "demo",
            Some("Demo Skill"),
            Some("2.0.0"),
            "# Demo Skill\nnew",
        );
        let target_root = tempfile::tempdir().unwrap();
        let agents = vec![
            AgentProfile {
                id: "agent-a".into(),
                name: "Agent A".into(),
                agent_type: crate::models::AgentType::Custom,
                skills_path: agent_a_root.path().to_string_lossy().to_string(),
                adapter_config: None,
            },
            AgentProfile {
                id: "agent-b".into(),
                name: "Agent B".into(),
                agent_type: crate::models::AgentType::Custom,
                skills_path: agent_b_root.path().to_string_lossy().to_string(),
                adapter_config: None,
            },
            AgentProfile {
                id: "target".into(),
                name: "Target".into(),
                agent_type: crate::models::AgentType::Custom,
                skills_path: target_root.path().to_string_lossy().to_string(),
                adapter_config: None,
            },
        ];
        for agent in agents {
            service.add_agent(agent).unwrap();
        }

        let results = service
            .sync_grouped_skill(
                "Demo Skill",
                None,
                vec!["target".into()],
                ConflictPolicy::BackupOverwrite,
            )
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "installed");
        assert_eq!(
            fs::read_to_string(target_root.path().join("demo").join("SKILL.md")).unwrap(),
            "# Demo Skill\nnew"
        );
    }

    #[test]
    fn skips_hidden_directories_when_scanning_agent_skills() {
        let root = tempfile::tempdir().unwrap();
        write_agent_skill(
            root.path(),
            "real-skill",
            Some("Real Skill"),
            Some("1.0.0"),
            "# Real",
        );
        fs::create_dir_all(root.path().join(".system")).unwrap();
        fs::write(root.path().join(".system").join("config.json"), "{}").unwrap();
        fs::create_dir_all(root.path().join(".hidden")).unwrap();

        let agent = AgentProfile {
            id: "test".into(),
            name: "Test".into(),
            agent_type: crate::models::AgentType::Custom,
            skills_path: root.path().to_string_lossy().to_string(),
            adapter_config: None,
        };
        let copies = scan_agent_skill_copies(&agent).unwrap();
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].title, "Real Skill");
    }
}
