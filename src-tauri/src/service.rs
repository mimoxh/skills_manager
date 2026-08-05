use crate::{
    adapter::{AgentAdapter, adapter_for, built_in_adapters},
    catalog::{
        CLAWHUB_API_CACHE_FILE, scan_catalog_repository, scan_clawhub_api_cache,
        sort_catalog_skills,
    },
    catalog_index::CatalogIndex,
    catalog_refresh::{built_in_catalog_sources, refresh_key},
    cherry_studio::CherryStudioAdapter,
    error::{AppError, AppResult},
    hash::{copy_dir_all, hash_dir},
    manifest::{read_skill, scan_repository, scan_skill_md_only, synthesize_manifest_from_skill_md},
    mcp_service::McpService,
    models::{
        AgentProfile, AgentType, CatalogFilters, CatalogInstallStatus, CatalogRefreshResult,
        CatalogRefreshStatus, CatalogSafetyMode, CatalogSearchResult, CatalogSkill, CatalogSort,
        CatalogSource, CatalogSourceKind, ConflictPolicy, GroupedSkill, ImportSkillFile,
        ImportSkillResult, InitialData, InstallResult,
    },
    skill_scan::{
        group_agent_skills, read_agent_skill_readme, register_claude_cowork_skill,
        scan_agent_skill_copies,
    },
    store::{AppStore, InstallRecordInput},
    util::{
        catalog_matches_filters, catalog_matches_query, catalog_skill_is_installed,
        command_no_window, normalize_title, page_catalog_skills, safe_label, safe_relative_path,
    },
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct AppService {
    pub(crate) store: Arc<AppStore>,
    pub(crate) mcp_service: Arc<McpService>,
    pub(crate) catalog_index: Arc<CatalogIndex>,
    pub(crate) catalog_refresh_cancel: Arc<Mutex<HashSet<String>>>,
    /// 已安装集合缓存（installed_titles, installed_slugs）。
    /// search_catalog_skills 只需算一次，变更点（安装/同步/导入/卸载/回滚/agent 增删）时失效。
    installed_cache: Arc<Mutex<Option<(HashSet<String>, HashSet<String>)>>>,
}

impl AppService {
    pub fn new() -> AppResult<Self> {
        let store = Arc::new(AppStore::new()?);
        let catalog_index = Arc::new(CatalogIndex::new(&store.data_dir())?);
        Ok(Self {
            store: Arc::clone(&store),
            mcp_service: Arc::new(McpService::new(&store)),
            catalog_index,
            catalog_refresh_cancel: Arc::new(Mutex::new(HashSet::new())),
            installed_cache: Arc::new(Mutex::new(None)),
        })
    }

    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let store = Arc::new(AppStore::in_memory()?);
        Ok(Self {
            store: Arc::clone(&store),
            mcp_service: Arc::new(McpService::new(&store)),
            catalog_index: Arc::new(CatalogIndex::new(&std::env::temp_dir().join(format!(
                "skill-sync-manager-test-index-{}",
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
            )))?),
            catalog_refresh_cancel: Arc::new(Mutex::new(HashSet::new())),
            installed_cache: Arc::new(Mutex::new(None)),
        })
    }

    pub fn store(&self) -> &AppStore {
        self.store.as_ref()
    }

    pub fn mcp(&self) -> &McpService {
        self.mcp_service.as_ref()
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

    /// 解析安装冲突，返回 (目标路径, action, 备份路径)。
    /// - 目标不存在：直接安装，action = "installed"。
    /// - Prompt：返回 Err（需用户先决策）。
    /// - Skip：返回 None（调用方跳过该 agent）。
    /// - Rename：目标改名（`{名}-{时间戳}`），action = "renamed"。
    /// - BackupOverwrite：备份到 data_dir/backups 后清空目标（Cowork 除外，避免破坏 manifest），action = "updated"。
    /// 三条安装路径（安装/同步/导入）共用，消除三份冲突处理逻辑重复。
    fn resolve_install_conflict(
        &self,
        agent: &AgentProfile,
        skills_path: &Path,
        target_dir_name: &str,
        skill_label: &str,
        conflict_policy: &ConflictPolicy,
    ) -> AppResult<Option<(PathBuf, String, Option<String>)>> {
        let target = skills_path.join(target_dir_name);
        if !target.exists() {
            return Ok(Some((target, "installed".to_string(), None)));
        }
        match conflict_policy {
            ConflictPolicy::Prompt => Err(AppError::Message(
                "目标已存在。请先选择备份覆盖、跳过冲突或另存副本策略。".to_string(),
            )),
            ConflictPolicy::Skip => Ok(None),
            ConflictPolicy::Rename => {
                let suffix = chrono::Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
                let renamed = skills_path.join(format!("{}-{}", target_dir_name, suffix));
                Ok(Some((renamed, "renamed".to_string(), None)))
            }
            ConflictPolicy::BackupOverwrite => {
                let backup = self
                    .backup_root()
                    .join(safe_label(&agent.id))
                    .join(safe_label(skill_label))
                    .join(chrono::Utc::now().format("%Y%m%d%H%M%S%3f").to_string());
                copy_dir_all(&target, &backup)?;
                if agent.agent_type != AgentType::ClaudeCowork {
                    fs::remove_dir_all(&target)?;
                }
                Ok(Some((
                    target,
                    "updated".to_string(),
                    Some(backup.to_string_lossy().to_string()),
                )))
            }
        }
    }

    pub fn detect_agents(&self) -> AppResult<Vec<AgentProfile>> {
        let mut agents = Vec::new();
        for adapter in built_in_adapters() {
            agents.extend(adapter.detect());
        }
        Ok(agents)
    }

    pub fn get_initial_data(&self) -> AppResult<InitialData> {
        let agents = match self.list_agents() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[skills_manager] get_initial_data list_agents failed: {e}");
                Vec::new()
            }
        };
        let skills = match self.scan_agent_skills() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[skills_manager] get_initial_data scan_agent_skills failed: {e}");
                Vec::new()
            }
        };
        let no_full_coverage_titles = match self.store.list_no_full_coverage() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[skills_manager] get_initial_data list_no_full_coverage failed: {e}");
                Vec::new()
            }
        };
        let no_full_coverage_mcp_titles = match self.store.list_no_full_coverage_mcp() {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "[skills_manager] get_initial_data list_no_full_coverage_mcp failed: {e}"
                );
                Vec::new()
            }
        };
        Ok(InitialData {
            skills,
            agents,
            no_full_coverage_titles,
            no_full_coverage_mcp_titles,
            default_catalog_source_id: built_in_catalog_sources()
                .into_iter()
                .next()
                .map(|source| source.id)
                .unwrap_or_else(|| "clawhub".to_string()),
        })
    }

    pub fn toggle_no_full_coverage(&self, title: &str) -> AppResult<bool> {
        self.store.toggle_no_full_coverage(title)
    }

    pub fn toggle_no_full_coverage_mcp(&self, title: &str) -> AppResult<bool> {
        self.store.toggle_no_full_coverage_mcp(title)
    }

    pub fn set_skill_tags(&self, title: &str, tags: Vec<String>) -> AppResult<Vec<String>> {
        self.store.set_skill_tags(title, tags)
    }

    pub fn set_agent_tags(&self, agent_id: &str, tags: Vec<String>) -> AppResult<Vec<String>> {
        self.store.set_agent_tags(agent_id, tags)
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
        for agent in &mut values {
            agent.user_tags = self.store.list_agent_tags(&agent.id)?;
        }
        values.sort_by(|a, b| a.name.cmp(&b.name).then(a.skills_path.cmp(&b.skills_path)));
        Ok(values)
    }

    pub fn add_agent(&self, profile: AgentProfile) -> AppResult<AgentProfile> {
        let adapter = adapter_for(&profile);
        adapter.validate(&profile)?;
        self.store.save_agent(&profile)?;
        self.invalidate_installed_cache();
        Ok(profile)
    }

    pub fn remove_agent(&self, agent_id: &str) -> AppResult<()> {
        self.store.remove_agent(agent_id)?;
        self.invalidate_installed_cache();
        Ok(())
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
                    .map(crate::util::system_time_to_rfc3339);
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

        if source.id == "clawhub" {
            // ClawHub 内置源经 startCatalogRefresh 后台刷新，避免在此同步阻塞主线程
            return Err(AppError::Message(
                "ClawHub 内置源请使用 startCatalogRefresh 后台刷新。".to_string(),
            ));
        }
        let skill_count = if cache_path.join(".git").is_dir() {
            let output = command_no_window("git")
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
            let output = command_no_window("git")
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

    pub fn start_catalog_refresh(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
    ) -> AppResult<CatalogRefreshStatus> {
        if source_id != "clawhub" {
            self.refresh_catalog_source(source_id)?;
            return self.get_catalog_refresh_status(source_id, safety_mode);
        }
        let key = refresh_key(source_id, safety_mode);
        {
            let mut cancel = self
                .catalog_refresh_cancel
                .lock()
                .map_err(|_| AppError::Message("Refresh cancel lock poisoned".to_string()))?;
            cancel.remove(&key);
        }
        self.catalog_index.begin_refresh("clawhub", safety_mode)?;
        let service = self.clone();
        let source_id = source_id.to_string();
        std::thread::spawn(move || {
            let _ = service.refresh_clawhub_index(safety_mode).map_err(|error| {
                if error.to_string() == "用户已取消刷新" {
                    return;
                }
                let _ = service.mark_clawhub_refresh_error(
                    &source_id,
                    safety_mode,
                    error.to_string().as_str(),
                );
            });
        });
        self.get_catalog_refresh_status("clawhub", safety_mode)
    }

    pub fn get_catalog_refresh_status(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
    ) -> AppResult<CatalogRefreshStatus> {
        self.catalog_index.refresh_status(source_id, safety_mode)
    }

    pub fn cancel_catalog_refresh(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
    ) -> AppResult<CatalogRefreshStatus> {
        let key = refresh_key(source_id, safety_mode);
        let mut cancel = self
            .catalog_refresh_cancel
            .lock()
            .map_err(|_| AppError::Message("Refresh cancel lock poisoned".to_string()))?;
        cancel.insert(key);
        drop(cancel);
        self.mark_catalog_refresh_cancelled(source_id, safety_mode)
    }

    /// 计算已安装集合（titles + slugs）。带缓存：search_catalog_skills 反复调用时避免全量重扫，
    /// 安装/同步/导入/卸载/回滚/agent 增删等变更点调用 invalidate_installed_cache 失效。
    fn installed_sets(&self) -> AppResult<(HashSet<String>, HashSet<String>)> {
        let cache_hit = {
            let cache = self
                .installed_cache
                .lock()
                .map_err(|_| AppError::Message("Installed cache lock poisoned".to_string()))?;
            cache.clone()
        };
        if let Some(sets) = cache_hit {
            return Ok(sets);
        }
        let installed = self.scan_agent_skills()?;
        let installed_titles = installed
            .iter()
            .map(|skill| normalize_title(&skill.title))
            .collect::<HashSet<_>>();
        let installed_slugs = installed
            .iter()
            .flat_map(|skill| skill.copies.iter())
            .filter_map(|copy| {
                Path::new(&copy.skill_path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(normalize_title)
            })
            .collect::<HashSet<_>>();
        let sets = (installed_titles, installed_slugs);
        if let Ok(mut cache) = self.installed_cache.lock() {
            *cache = Some(sets.clone());
        }
        Ok(sets)
    }

    /// 失效已安装集合缓存，在安装/同步/导入/卸载/回滚/agent 增删等变更后调用。
    fn invalidate_installed_cache(&self) {
        if let Ok(mut cache) = self.installed_cache.lock() {
            *cache = None;
        }
    }

    pub fn search_catalog_skills(
        &self,
        query: Option<&str>,
        sort: CatalogSort,
        filters: CatalogFilters,
        page: Option<usize>,
        page_size: Option<usize>,
    ) -> AppResult<CatalogSearchResult> {
        let sources = self.list_catalog_sources()?;
        let (installed_titles, installed_slugs) = self.installed_sets()?;
        let q = query.unwrap_or("").trim().to_ascii_lowercase();
        let mut skills = Vec::new();

        for source in sources.into_iter().filter(|source| source.enabled) {
            if !filters.source_ids.is_empty() && !filters.source_ids.contains(&source.id) {
                continue;
            }
            let cache_path = self.catalog_cache_path(&source);
            if !cache_path.exists() {
                if source.id != "clawhub" {
                    continue;
                }
            }
            let mut source_skills = if source.id == "clawhub" {
                if self
                    .catalog_index
                    .count("clawhub", filters.safety_mode)
                    .unwrap_or_default()
                    > 0
                {
                    // 取 ClawHub 全部匹配项，与其它源合并后统一在内存中过滤/排序/分页，
                    // 避免提前 return 导致 claude/codex/自定义 git 源永不参与搜索
                    self.catalog_index
                        .query(
                            &source,
                            &q,
                            sort.clone(),
                            &filters,
                            &installed_titles,
                            &installed_slugs,
                            1,
                            usize::MAX,
                        )?
                        .items
                } else if cache_path.join(CLAWHUB_API_CACHE_FILE).exists()
                    && filters.safety_mode == CatalogSafetyMode::All
                {
                    scan_clawhub_api_cache(&cache_path, &source)?
                } else {
                    Vec::new()
                }
            } else {
                scan_catalog_repository(&cache_path, &source)?
            };
            for skill in &mut source_skills {
                if catalog_skill_is_installed(skill, &installed_titles, &installed_slugs) {
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
        Ok(page_catalog_skills(
            sort_catalog_skills(filtered, sort),
            page,
            page_size,
        ))
    }

    /// 按完整 id 跨启用源直查 catalog skill（clawhub 走 SQL 索引，其余走仓库扫描），
    /// 避免通过"最近更新前 500"的搜索窗口查找导致老技能无法安装。
    fn find_catalog_skill_by_id(&self, catalog_skill_id: &str) -> AppResult<Option<CatalogSkill>> {
        let sources = self.list_catalog_sources()?;
        for source in sources.into_iter().filter(|source| source.enabled) {
            let cache_path = self.catalog_cache_path(&source);
            if source.id == "clawhub" {
                if let Some(found) = self.catalog_index.find_by_id(&source.id, catalog_skill_id)? {
                    return Ok(Some(found));
                }
            } else if cache_path.exists() {
                let scanned = scan_catalog_repository(&cache_path, &source)?;
                if let Some(found) = scanned
                    .into_iter()
                    .find(|skill| skill.id == catalog_skill_id)
                {
                    return Ok(Some(found));
                }
            }
        }
        Ok(None)
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
            .find_catalog_skill_by_id(catalog_skill_id)?
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
        let source_fingerprint = hash_dir(source_path)?;
        let agents = self.list_agents()?;
        let agent_map: HashMap<_, _> = agents
            .into_iter()
            .map(|agent| (agent.id.clone(), agent))
            .collect();
        let mut results = Vec::new();
        let mut install_records: Vec<InstallRecordInput> = Vec::new();

        // 先校验全部目标 agent 存在，再执行安装，避免循环中途失败留下半完成状态
        for agent_id in &target_agent_ids {
            if !agent_map.contains_key(agent_id) {
                return Err(AppError::Message(format!("找不到 Agent: {}", agent_id)));
            }
        }

        for agent_id in target_agent_ids {
            let agent = agent_map
                .get(&agent_id)
                .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;
            let skills_path = Path::new(&agent.skills_path);
            fs::create_dir_all(skills_path)?;
            // 逐 agent 失败隔离：单个 agent 安装失败记录为 error 结果，不中断整批
            let outcome = (|| -> AppResult<InstallResult> {
                let Some((target, action, backup_path)) = self.resolve_install_conflict(
                    agent,
                    skills_path,
                    source_dir_name,
                    &skill.name,
                    &conflict_policy,
                )? else {
                    return Ok(InstallResult {
                        agent_id: agent.id.clone(),
                        skill_id: skill.name.clone(),
                        action: "skipped".to_string(),
                        target_path: skills_path.join(source_dir_name).to_string_lossy().to_string(),
                        backup_path: None,
                        message: format!("已跳过 {}", skill.name),
                    });
                };
                if agent.agent_type == AgentType::CherryStudio {
                    let cs = CherryStudioAdapter::new().ok_or_else(|| {
                        AppError::Message(
                            "未找到 Cherry Studio 安装目录（%APPDATA%\\CherryStudio 缺失），无法安装。"
                                .to_string(),
                        )
                    })?;
                    // 复制到 Cherry Studio Skills 目录并在 agents.db 中注册，避免"文件在但技能不可见"
                    cs.install_skill(source_path, source_dir_name)?;
                } else {
                    copy_dir_all(source_path, &target)?;
                    if agent.agent_type == AgentType::ClaudeCowork {
                        let target_dir_name = target
                            .file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or(source_dir_name);
                        register_claude_cowork_skill(agent, target_dir_name, &target)?;
                    }
                }
                // 批量记录：先收集，循环结束后统一一次 save（M-B16）
                install_records.push(InstallRecordInput {
                    agent_id: agent.id.clone(),
                    skill_id: skill.name.clone(),
                    fingerprint: source_fingerprint.clone(),
                    target_path: target.to_string_lossy().to_string(),
                    action: action.clone(),
                    backup_path: backup_path.clone(),
                });
                let install_msg = match action.as_str() {
                    "updated" => format!("已更新 {} 到 {}", skill.name, agent.name),
                    "renamed" => format!("已另存副本 {} 到 {}", skill.name, agent.name),
                    _ => format!("已安装 {} 到 {}", skill.name, agent.name),
                };
                Ok(InstallResult {
                    agent_id: agent.id.clone(),
                    skill_id: skill.name.clone(),
                    action,
                    target_path: target.to_string_lossy().to_string(),
                    backup_path,
                    message: install_msg,
                })
            })();
            match outcome {
                Ok(result) => results.push(result),
                Err(error) => results.push(InstallResult {
                    agent_id: agent.id.clone(),
                    skill_id: skill.name.clone(),
                    action: "error".to_string(),
                    target_path: skills_path.join(source_dir_name).to_string_lossy().to_string(),
                    backup_path: None,
                    message: format!("安装 {} 到 {} 失败: {}", skill.name, agent.name, error),
                }),
            }
        }
        if !install_records.is_empty() {
            self.store.record_installs(&install_records)?;
        }
        self.invalidate_installed_cache();
        Ok(results)
    }

    pub fn scan_agent_skills(&self) -> AppResult<Vec<GroupedSkill>> {
        let agents = self.list_agents()?;
        let mut copies = Vec::new();
        for agent in &agents {
            copies.extend(scan_agent_skill_copies(agent)?);
        }
        let mut groups = group_agent_skills(&agents, copies);
        for group in &mut groups {
            group.user_tags = self.store.list_skill_tags(&group.title)?;
        }
        Ok(groups)
    }

    pub fn read_agent_skill_readme(&self, skill_path: &str) -> AppResult<Option<String>> {
        Ok(read_agent_skill_readme(Path::new(skill_path)))
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
        let mut install_records: Vec<InstallRecordInput> = Vec::new();

        // 先校验全部目标 agent 存在，再执行安装，避免循环中途失败留下半完成状态
        for agent_id in &target_agent_ids {
            if !agent_map.contains_key(agent_id) {
                return Err(AppError::Message(format!("找不到 Agent: {}", agent_id)));
            }
        }

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
            let skills_path = Path::new(&agent.skills_path);
            fs::create_dir_all(skills_path)?;
            // 逐 agent 失败隔离：单个 agent 同步失败记录为 error 结果，不中断整批
            let outcome = (|| -> AppResult<InstallResult> {
                let Some((target, action, backup_path)) = self.resolve_install_conflict(
                    agent,
                    skills_path,
                    source_dir_name,
                    title,
                    &conflict_policy,
                )? else {
                    return Ok(InstallResult {
                        agent_id: agent.id.clone(),
                        skill_id: title.to_string(),
                        action: "skipped".to_string(),
                        target_path: skills_path.join(source_dir_name).to_string_lossy().to_string(),
                        backup_path: None,
                        message: format!("已跳过 {}", title),
                    });
                };

                if agent.agent_type == AgentType::CherryStudio {
                    let cs = CherryStudioAdapter::new().ok_or_else(|| {
                        AppError::Message(
                            "未找到 Cherry Studio 安装目录（%APPDATA%\\CherryStudio 缺失），无法安装。"
                                .to_string(),
                        )
                    })?;
                    cs.install_skill(source_path, source_dir_name)?;
                } else if agent.agent_type == AgentType::ClaudeCowork {
                    copy_dir_all(source_path, &target)?;
                    register_claude_cowork_skill(agent, source_dir_name, &target)?;
                } else {
                    copy_dir_all(source_path, &target)?;
                }
                // 批量记录：先收集，循环结束后统一一次 save（M-B16）
                install_records.push(InstallRecordInput {
                    agent_id: agent.id.clone(),
                    skill_id: title.to_string(),
                    fingerprint: source_fingerprint.clone(),
                    target_path: target.to_string_lossy().to_string(),
                    action: action.clone(),
                    backup_path: backup_path.clone(),
                });
                let sync_msg = match action.as_str() {
                    "updated" => format!("已更新 {} 到 {}", title, agent.name),
                    "renamed" => format!("已另存副本 {} 到 {}", title, agent.name),
                    _ => format!("{} 已同步到 {}", title, agent.name),
                };
                Ok(InstallResult {
                    agent_id: agent.id.clone(),
                    skill_id: title.to_string(),
                    action,
                    target_path: target.to_string_lossy().to_string(),
                    backup_path,
                    message: sync_msg,
                })
            })();
            match outcome {
                Ok(result) => results.push(result),
                Err(error) => results.push(InstallResult {
                    agent_id: agent.id.clone(),
                    skill_id: title.to_string(),
                    action: "error".to_string(),
                    target_path: skills_path.join(source_dir_name).to_string_lossy().to_string(),
                    backup_path: None,
                    message: format!("同步 {} 到 {} 失败: {}", title, agent.name, error),
                }),
            }
        }
        if !install_records.is_empty() {
            self.store.record_installs(&install_records)?;
        }
        self.invalidate_installed_cache();
        Ok(results)
    }

    pub fn uninstall_skill(&self, skill_id: &str, agent_id: &str) -> AppResult<()> {
        let agent = self
            .list_agents()?
            .into_iter()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;

        let matched_path = scan_agent_skill_copies(&agent)?
            .into_iter()
            .find(|copy| normalize_title(&copy.title) == normalize_title(skill_id))
            .map(|copy| PathBuf::from(copy.skill_path));
        let target_path = match matched_path {
            Some(path) => path,
            None => {
                // 回退路径：skill_id 可能来自用户输入，先用 safe_relative_path 校验，防止 .. / 绝对路径逃逸出 skills_path
                let safe = safe_relative_path(skill_id)?;
                Path::new(&agent.skills_path).join(safe)
            }
        };
        let target_name = target_path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(skill_id);

        if agent.agent_type == AgentType::CherryStudio {
            let cs = CherryStudioAdapter::new().ok_or_else(|| {
                AppError::Message(
                    "未找到 Cherry Studio 安装目录（%APPDATA%\\CherryStudio 缺失），无法卸载。"
                        .to_string(),
                )
            })?;
            cs.uninstall_skill(target_name)?;
        } else {
            let adapter = adapter_for(&agent);
            adapter.uninstall(target_name, &agent)?;
        }

        self.store
            .record_uninstall(agent_id, skill_id, &target_path.to_string_lossy(), None)?;
        self.invalidate_installed_cache();
        Ok(())
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
        self.invalidate_installed_cache();
        Ok(())
    }

    pub fn repair_claude_cowork_manifest(&self, agent_id: &str) -> AppResult<ImportSkillResult> {
        let agent = self
            .list_agents()?
            .into_iter()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;
        if agent.agent_type != AgentType::ClaudeCowork {
            return Err(AppError::Message(
                "只能修复 Claude Desktop Cowork 清单".to_string(),
            ));
        }

        let mut repaired = 0usize;
        for copy in scan_agent_skill_copies(&agent)? {
            if copy.is_registered {
                continue;
            }
            let skill_path = PathBuf::from(&copy.skill_path);
            let Some(skill_id) = skill_path
                .file_name()
                .and_then(|value| value.to_str())
                .filter(|value| !value.trim().is_empty())
            else {
                continue;
            };
            register_claude_cowork_skill(&agent, skill_id, &skill_path)?;
            repaired += 1;
        }

        Ok(ImportSkillResult {
            imported: repaired,
            skipped: 0,
            message: format!("已修复 {} 个 Cowork manifest 条目。", repaired),
        })
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
        let mut dirs = self.manifest_source_dirs(source_root)?;
        let mut using_skill_md_fallback = false;

        // Fallback: scan for SKILL.md-only directories when no manifest files found
        if dirs.is_empty() {
            let skill_md_skills = scan_skill_md_only(source_root)?;
            if !skill_md_skills.is_empty() {
                using_skill_md_fallback = true;
                for skill in &skill_md_skills {
                    dirs.push(PathBuf::from(&skill.source_path));
                }
            }
        }

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
            let skill = if using_skill_md_fallback {
                let skill_md = source.join("SKILL.md");
                synthesize_manifest_from_skill_md(&skill_md)?
            } else {
                read_skill(&self.manifest_path_for(source)?)?
            };
            let skill_dir_name = source
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or_else(|| AppError::Message("skill 目录名无效".to_string()))?;

            for agent_id in target_agent_ids {
                let agent = agent_map
                    .get(agent_id)
                    .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;
                let skills_path = Path::new(&agent.skills_path);
                fs::create_dir_all(skills_path)?;
                // 逐 agent 失败隔离：单个 agent 导入失败计入 skipped 并继续，不中断整批
                let outcome = (|| -> AppResult<()> {
                    let Some((target, _action, _backup_path)) = self.resolve_install_conflict(
                        agent,
                        skills_path,
                        skill_dir_name,
                        &skill.manifest.id,
                        &conflict_policy,
                    )? else {
                        skipped += 1;
                        return Ok(());
                    };

                    if agent.agent_type == AgentType::CherryStudio {
                        let cs = CherryStudioAdapter::new().ok_or_else(|| {
                            AppError::Message(
                                "未找到 Cherry Studio 安装目录（%APPDATA%\\CherryStudio 缺失），无法导入。"
                                    .to_string(),
                            )
                        })?;
                        cs.install_skill(source, skill_dir_name)?;
                    } else if agent.agent_type == AgentType::ClaudeCowork {
                        copy_dir_all(source, &target)?;
                        let target_dir_name = target
                            .file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or(skill_dir_name);
                        register_claude_cowork_skill(agent, target_dir_name, &target)?;
                    } else {
                        copy_dir_all(source, &target)?;
                    }
                    imported += 1;
                    Ok(())
                })();
                if outcome.is_err() {
                    skipped += 1;
                }
            }
        }

        self.invalidate_installed_cache();
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

    pub(crate) fn import_workspace(&self, label: &str) -> AppResult<PathBuf> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_index::RefreshStatePatch;
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
            user_tags: Vec::new(),
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

    #[test]
    fn cancel_catalog_refresh_immediately_marks_state_not_running() {
        let service = AppService::in_memory().unwrap();
        let generation = service
            .catalog_index
            .begin_refresh("clawhub", CatalogSafetyMode::All)
            .unwrap();
        service
            .catalog_index
            .save_refresh_state(RefreshStatePatch {
                source_id: "clawhub",
                safety_mode: CatalogSafetyMode::All,
                cursor: Some("cursor-1"),
                fetched_count: 200,
                generation,
                is_running: true,
                is_complete: false,
                last_error: None,
            })
            .unwrap();

        let status = service
            .cancel_catalog_refresh("clawhub", CatalogSafetyMode::All)
            .unwrap();

        assert!(!status.is_running);
        assert!(!status.is_complete);
        assert_eq!(status.next_cursor.as_deref(), Some("cursor-1"));
        assert_eq!(status.fetched_count, 200);
        assert_eq!(status.generation, generation);
        assert_eq!(status.last_error.as_deref(), Some("用户已取消刷新"));

        let persisted = service
            .get_catalog_refresh_status("clawhub", CatalogSafetyMode::All)
            .unwrap();
        assert_eq!(persisted, status);
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

    fn create_cowork_agent(service: &AppService, root: &Path) -> AgentProfile {
        let plugin_root = root.join("cowork-plugin");
        fs::create_dir_all(plugin_root.join(".claude-plugin")).unwrap();
        fs::create_dir_all(plugin_root.join("skills")).unwrap();
        fs::write(plugin_root.join(".claude-plugin").join("plugin.json"), "{}").unwrap();
        fs::write(
            plugin_root.join("manifest.json"),
            serde_json::json!({
                "lastUpdated": 1781697450424u64,
                "skills": []
            })
            .to_string(),
        )
        .unwrap();
        let profile = AgentProfile {
            id: "cowork-agent".into(),
            name: "Claude Desktop Cowork".into(),
            agent_type: crate::models::AgentType::ClaudeCowork,
            skills_path: plugin_root.join("skills").to_string_lossy().to_string(),
            adapter_config: Some(serde_json::json!({
                "pluginRoot": plugin_root.to_string_lossy(),
                "manifestPath": plugin_root.join("manifest.json").to_string_lossy()
            })),
            user_tags: Vec::new(),
        };
        service.add_agent(profile.clone()).unwrap();
        profile
    }

    fn cowork_manifest(profile: &AgentProfile) -> serde_json::Value {
        let manifest_path = profile
            .adapter_config
            .as_ref()
            .and_then(|value| value.get("manifestPath"))
            .and_then(|value| value.as_str())
            .unwrap();
        serde_json::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap()
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
    fn cloned_service_shares_store_state() {
        let service = AppService::in_memory().unwrap();
        let clone = service.clone();
        let agent_dir = tempfile::tempdir().unwrap();
        let profile = AgentProfile {
            id: "shared-agent".into(),
            name: "Shared Agent".into(),
            agent_type: crate::models::AgentType::Custom,
            skills_path: agent_dir.path().to_string_lossy().to_string(),
            adapter_config: None,
            user_tags: Vec::new(),
        };

        service.add_agent(profile).unwrap();

        let agents = clone.list_saved_agents().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, "shared-agent");
    }

    #[test]
    fn list_agents_includes_user_tags_from_store() {
        let service = AppService::in_memory().unwrap();
        let agent_dir = tempfile::tempdir().unwrap();
        let profile = AgentProfile {
            id: "tagged-agent".into(),
            name: "Tagged Agent".into(),
            agent_type: crate::models::AgentType::Custom,
            skills_path: agent_dir.path().to_string_lossy().to_string(),
            adapter_config: None,
            user_tags: Vec::new(),
        };

        service.add_agent(profile).unwrap();
        service
            .set_agent_tags(
                "tagged-agent",
                vec!["生产力".to_string(), "AI".to_string()],
            )
            .unwrap();

        let agents = service.list_agents().unwrap();
        let tagged = agents
            .into_iter()
            .find(|agent| agent.id == "tagged-agent")
            .unwrap();
        assert_eq!(tagged.user_tags, vec!["生产力".to_string(), "AI".to_string()]);
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
    fn scan_agent_skills_includes_user_tags_from_store() {
        let agent_dir = tempfile::tempdir().unwrap();
        let service = test_service_with_agent(agent_dir.path());
        write_agent_skill(
            agent_dir.path(),
            "demo-skill",
            Some("Demo Skill"),
            Some("1.0.0"),
            "# Demo Skill",
        );

        service
            .set_skill_tags(" demo skill ", vec!["AI".to_string(), "效率".to_string()])
            .unwrap();

        let skills = service.scan_agent_skills().unwrap();
        let demo = skills
            .into_iter()
            .find(|skill| skill.title == "Demo Skill")
            .unwrap();
        assert_eq!(demo.user_tags, vec!["AI".to_string(), "效率".to_string()]);
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
                user_tags: Vec::new(),
            },
            AgentProfile {
                id: "agent-b".into(),
                name: "Agent B".into(),
                agent_type: crate::models::AgentType::Custom,
                skills_path: agent_b_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
            },
            AgentProfile {
                id: "target".into(),
                name: "Target".into(),
                agent_type: crate::models::AgentType::Custom,
                skills_path: target_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
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
    fn syncs_grouped_skill_isolates_per_agent_failure() {
        let service = AppService::in_memory().unwrap();
        let source_root = tempfile::tempdir().unwrap();
        write_agent_skill(
            source_root.path(),
            "demo",
            Some("Demo Skill"),
            Some("1.0.0"),
            "# Demo Skill",
        );
        service
            .add_agent(AgentProfile {
                id: "source".into(),
                name: "Source".into(),
                agent_type: crate::models::AgentType::Custom,
                skills_path: source_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
            })
            .unwrap();
        let ok_root = tempfile::tempdir().unwrap();
        // bad-agent 的 skills 目录里预置一个同名"demo"文件：BackupOverwrite 时 remove_dir_all 失败 → 单 agent 隔离
        let bad_root = tempfile::tempdir().unwrap();
        fs::write(bad_root.path().join("demo"), "occupied").unwrap();
        for (id, path) in [
            ("ok-agent", ok_root.path().to_string_lossy().to_string()),
            ("bad-agent", bad_root.path().to_string_lossy().to_string()),
        ] {
            service
                .add_agent(AgentProfile {
                    id: id.into(),
                    name: id.into(),
                    agent_type: crate::models::AgentType::Custom,
                    skills_path: path,
                    adapter_config: None,
                    user_tags: Vec::new(),
                })
                .unwrap();
        }

        let results = service
            .sync_grouped_skill(
                "Demo Skill",
                Some("source"),
                vec!["ok-agent".into(), "bad-agent".into()],
                ConflictPolicy::BackupOverwrite,
            )
            .unwrap();

        // 两个 agent 都返回结果：ok-agent 成功安装，bad-agent 隔离为 error
        assert_eq!(results.len(), 2);
        let ok = results.iter().find(|r| r.agent_id == "ok-agent").unwrap();
        assert_eq!(ok.action, "installed");
        let bad = results.iter().find(|r| r.agent_id == "bad-agent").unwrap();
        assert_eq!(bad.action, "error");
    }

    #[test]
    fn syncs_skill_to_claude_cowork_and_registers_manifest() {
        let service = AppService::in_memory().unwrap();
        let source_root = tempfile::tempdir().unwrap();
        write_agent_skill(
            source_root.path(),
            "academic-paper",
            Some("academic-paper"),
            Some("3.1.1"),
            "---\nname: academic-paper\ndescription: 学术论文写作流水线。\n---\n# Academic Paper",
        );
        service
            .add_agent(AgentProfile {
                id: "source".into(),
                name: "Source".into(),
                agent_type: crate::models::AgentType::Custom,
                skills_path: source_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
            })
            .unwrap();
        let cowork_root = tempfile::tempdir().unwrap();
        let cowork = create_cowork_agent(&service, cowork_root.path());

        let first = service
            .sync_grouped_skill(
                "academic-paper",
                Some("source"),
                vec![cowork.id.clone()],
                ConflictPolicy::BackupOverwrite,
            )
            .unwrap();
        let second = service
            .sync_grouped_skill(
                "academic-paper",
                Some("source"),
                vec![cowork.id.clone()],
                ConflictPolicy::BackupOverwrite,
            )
            .unwrap();

        assert_eq!(first[0].action, "installed");
        assert_eq!(second[0].action, "updated");
        assert!(
            Path::new(&cowork.skills_path)
                .join("academic-paper")
                .join("SKILL.md")
                .exists()
        );
        let manifest = cowork_manifest(&cowork);
        let skills = manifest.get("skills").unwrap().as_array().unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(
            skills[0].get("skillId").and_then(|value| value.as_str()),
            Some("academic-paper")
        );
        assert_eq!(
            skills[0].get("name").and_then(|value| value.as_str()),
            Some("academic-paper")
        );
        assert_eq!(
            skills[0]
                .get("creatorType")
                .and_then(|value| value.as_str()),
            Some("user")
        );
        assert_eq!(
            skills[0]
                .get("syncManaged")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            skills[0].get("enabled").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(
            skills[0]
                .get("updatedAt")
                .and_then(|value| value.as_str())
                .is_some()
        );
    }

    #[test]
    fn scans_claude_cowork_directory_entries_as_unregistered_when_manifest_missing() {
        let service = AppService::in_memory().unwrap();
        let cowork_root = tempfile::tempdir().unwrap();
        let cowork = create_cowork_agent(&service, cowork_root.path());
        write_agent_skill(
            Path::new(&cowork.skills_path),
            "loose-skill",
            Some("Loose Skill"),
            Some("1.0.0"),
            "# Loose Skill",
        );

        let copy = scan_agent_skill_copies(&cowork)
            .unwrap()
            .into_iter()
            .find(|copy| copy.title == "Loose Skill")
            .unwrap();

        assert!(!copy.is_registered);
    }

    #[test]
    fn repairs_claude_cowork_manifest_for_existing_skill_directories() {
        let service = AppService::in_memory().unwrap();
        let cowork_root = tempfile::tempdir().unwrap();
        let cowork = create_cowork_agent(&service, cowork_root.path());
        write_agent_skill(
            Path::new(&cowork.skills_path),
            "loose-skill",
            Some("Loose Skill"),
            Some("1.0.0"),
            "---\nname: Loose Skill\ndescription: repaired description\n---\n# Loose Skill",
        );

        let result = service.repair_claude_cowork_manifest(&cowork.id).unwrap();

        assert_eq!(result.imported, 1);
        let manifest = cowork_manifest(&cowork);
        let skills = manifest.get("skills").unwrap().as_array().unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(
            skills[0].get("skillId").and_then(|value| value.as_str()),
            Some("loose-skill")
        );
        assert_eq!(
            skills[0].get("name").and_then(|value| value.as_str()),
            Some("Loose Skill")
        );
        assert_eq!(
            skills[0]
                .get("description")
                .and_then(|value| value.as_str()),
            Some("repaired description")
        );
    }

    #[test]
    fn uninstalls_grouped_skill_by_title_when_directory_name_differs() {
        let agent_dir = tempfile::tempdir().unwrap();
        let service = test_service_with_agent(agent_dir.path());
        write_agent_skill(
            agent_dir.path(),
            "powerpoint-pptx",
            Some("Powerpoint / PPTX"),
            Some("1.0.1"),
            "# Powerpoint / PPTX",
        );

        service
            .uninstall_skill_from_agents("Powerpoint / PPTX", &["test-agent".into()])
            .unwrap();

        assert!(!agent_dir.path().join("powerpoint-pptx").exists());
    }
}
