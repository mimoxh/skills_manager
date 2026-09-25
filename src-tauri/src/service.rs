use crate::{
    adapter::{AgentAdapter, adapter_for, default_skills_path},
    catalog::{
        CLAWHUB_API_CACHE_FILE, scan_catalog_repository, scan_clawhub_api_cache,
        sort_catalog_skills,
    },
    catalog_index::CatalogIndex,
    catalog_refresh::{built_in_catalog_sources, refresh_key},
    cherry_studio::CherryStudioAdapter,
    error::{AppError, AppResult},
    hash::{copy_dir_all, hash_dir, is_symlink_path, read_symlink_target, remove_dir_or_symlink, symlink_or_copy_dir},
    manifest::{read_skill, scan_repository, scan_skill_md_only, synthesize_manifest_from_skill_md},
    mcp_service::McpService,
    models::{
        AgentProfile, AgentType, CatalogFilters, CatalogInstallStatus, CatalogRefreshResult,
        CatalogRefreshStatus, CatalogSafetyMode, CatalogSearchResult, CatalogSkill, CatalogSort,
        CatalogSource, CatalogSourceKind, ConflictPolicy, GroupedSkill, ImportSkillFile,
        ImportSkillResult, InitialData, InstallResult, RemoteSourceInspection, SyncConfig, SyncConflictChoice, SyncStatus,
    },
    skill_scan::{
        group_agent_skills, load_skill_lock_map, read_agent_skill_info, read_agent_skill_readme,
        register_claude_cowork_skill, scan_agent_skill_copies,
        scan_agent_skill_copies_with_lock, write_skill_lock_entry,
    },
    store::{AppStore, InstallRecordInput, ManagedTargetRecord, PendingHubSkill},
    sync::{
        ensure_kdf_salt, is_local_endpoint, local_root_from_endpoint, Crypto, HubWatcher,
        KeyringSecretStore, LocalDirTransport, S3Transport, SecretStore, SyncManager, SyncRuntime,
        SyncTransport, ENCRYPT_PASSWORD, SECRET_ACCESS_KEY,
    },
    util::{
        catalog_matches_filters, catalog_matches_query, catalog_skill_is_installed,
        command_no_window, expand_user_path, is_universal_skills_path, normalize_title,
        page_catalog_skills, safe_label, safe_relative_path,
    },
};
#[cfg(not(test))]
use crate::util::{is_legacy_universal_skills_path, program_skills_path};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// 按配置构造同步传输：本地目录或 S3。
pub(crate) fn build_transport_from(
    config: &SyncConfig,
    secrets: &dyn SecretStore,
) -> AppResult<Box<dyn SyncTransport>> {
    if let Some(root) = local_root_from_endpoint(&config.endpoint) {
        let root = if config.bucket.trim().is_empty() {
            root
        } else {
            root.join(config.bucket.trim())
        };
        return Ok(Box::new(LocalDirTransport::new(root)?));
    }
    let secret = secrets
        .get(SECRET_ACCESS_KEY)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Message("未配置 S3 Secret Key。".to_string()))?;
    Ok(Box::new(S3Transport::new(
        &config.endpoint,
        &config.bucket,
        &config.region,
        &config.access_key_id,
        &secret,
    )?))
}

/// 校验同步传输所需配置（本地目录只要求 endpoint；S3 还要 bucket）。
pub(crate) fn ensure_sync_transport_ready(config: &SyncConfig) -> AppResult<()> {
    if !config.enabled {
        return Err(AppError::Message("同步未启用。".to_string()));
    }
    if is_local_endpoint(&config.endpoint) {
        return Ok(());
    }
    if config.endpoint.trim().is_empty() || config.bucket.trim().is_empty() {
        return Err(AppError::Message(
            "请先配置同步 endpoint 与 bucket。".to_string(),
        ));
    }
    Ok(())
}

/// 保留旧中枢原件；新目录已有同名内容时以新目录为准，避免迁移覆盖用户数据。
fn migrate_hub_contents(old_root: &Path, new_root: &Path, allowed: &HashSet<String>) -> AppResult<Vec<String>> {
    if !old_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut migrated_skills = Vec::new();
    for entry in fs::read_dir(old_root)? {
        let entry = entry?;
        let source = entry.path();
        if !allowed.contains(&entry.file_name().to_string_lossy().to_string()) { continue; }
        let destination = new_root.join(entry.file_name());
        if fs::symlink_metadata(&destination).is_ok() {
            continue;
        }
        let staging = new_root.join(format!(".migration-{}", uuid::Uuid::new_v4()));
        let copy_result = if source.is_dir() {
            copy_dir_all(&source, &staging)
        } else if source.is_file() {
            fs::copy(&source, &staging).map(|_| ()).map_err(Into::into)
        } else {
            continue;
        };
        if let Err(error) = copy_result {
            let _ = remove_dir_or_symlink(&staging);
            return Err(error);
        }
        if let Err(error) = fs::rename(&staging, &destination) {
            let _ = remove_dir_or_symlink(&staging);
            return Err(error.into());
        }
        if source.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                if !name.starts_with('.') {
                    migrated_skills.push(name.to_string());
                }
            }
        }
    }
    Ok(migrated_skills)
}

#[derive(Clone)]
pub struct AppService {
    pub(crate) store: Arc<AppStore>,
    pub(crate) mcp_service: Arc<McpService>,
    pub(crate) catalog_index: Arc<CatalogIndex>,
    pub(crate) catalog_refresh_cancel: Arc<Mutex<HashSet<String>>>,
    /// 已安装集合缓存（installed_titles, installed_slugs）。
    /// search_catalog_skills 只需算一次，变更点（安装/同步/导入/卸载/回滚/agent 增删）时失效。
    installed_cache: Arc<Mutex<Option<(HashSet<String>, HashSet<String>)>>>,
    /// 机密存储（S3 secret / 加密口令），默认走 OS 钥匙串。
    pub(crate) secrets: Arc<dyn SecretStore>,
    /// 后台同步执行器（串行化 + 兜底轮询）。
    pub(crate) sync_manager: Arc<SyncManager>,
    /// 中枢目录文件监听（启用同步时存在）。
    pub(crate) sync_watcher: Arc<Mutex<Option<HubWatcher>>>,
    /// 仅供单机双设备演示隔离中枢；桌面、CLI、MCP 均使用 exe 旁路径。
    demo_hub_path: Option<PathBuf>,
}

impl AppService {
    pub fn new() -> AppResult<Self> {
        let store = Arc::new(AppStore::new()?);
        Self::from_store(store, Arc::new(KeyringSecretStore::new("skill-sync-manager")))
    }

    /// 指定数据目录启动（同机双开做同步实验 / 便携实例）。
    pub fn with_data_dir(data_dir: PathBuf) -> AppResult<Self> {
        let store = Arc::new(AppStore::with_data_dir(data_dir)?);
        Self::from_store(store, Arc::new(KeyringSecretStore::new("skill-sync-manager")))
    }

    pub fn from_store(store: Arc<AppStore>, secrets: Arc<dyn SecretStore>) -> AppResult<Self> {
        let catalog_index = Arc::new(CatalogIndex::new(&store.data_dir())?);
        let service = Self {
            store: Arc::clone(&store),
            mcp_service: Arc::new(McpService::new(&store)),
            catalog_index,
            catalog_refresh_cancel: Arc::new(Mutex::new(HashSet::new())),
            installed_cache: Arc::new(Mutex::new(None)),
            secrets,
            sync_manager: Arc::new(SyncManager::new()),
            sync_watcher: Arc::new(Mutex::new(None)),
            demo_hub_path: None,
        };
        // 已启用同步则随启动拉起后台轮询线程与文件监听。
        if let Ok(config) = service.store.sync_config() {
            if config.enabled {
                service
                    .sync_manager
                    .start(service.clone(), config.poll_secs);
                service.start_sync_watcher();
            }
        }
        Ok(service)
    }

    /// 仅供 sync_local_demo：两个服务实例各用临时中枢，不影响正式应用的固定路径。
    #[doc(hidden)]
    pub fn with_demo_hub_path(mut self, path: PathBuf) -> Self {
        self.demo_hub_path = Some(path);
        self
    }

    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let store = Arc::new(AppStore::in_memory()?);
        Self::from_store(store, Arc::new(crate::sync::MemorySecretStore::new()))
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
        // 用 symlink_metadata：断链/纯链接节点也视为「已存在」，避免 exists() 跟随后误判为空
        if fs::symlink_metadata(&target).is_err() {
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
                fs::create_dir_all(&backup)?;
                if is_symlink_path(&target) {
                    let dest = read_symlink_target(&target)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    fs::write(backup.join(".symlink-target"), dest)?;
                } else {
                    copy_dir_all(&target, &backup)?;
                }
                if agent.agent_type != AgentType::ClaudeCowork {
                    remove_dir_or_symlink(&target)?;
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
        // 测试环境不扫描真实机器目录，避免污染 ~/.agents 与并行测试串扰
        #[cfg(test)]
        {
            return Ok(Vec::new());
        }
        #[cfg(not(test))]
        {
            if self.demo_hub_path.is_some() { return Ok(Vec::new()); }
            let mut agents = Vec::new();
            for adapter in crate::adapter::built_in_adapters() {
                agents.extend(adapter.detect());
            }
            Ok(agents)
        }
    }

    pub fn get_initial_data(&self) -> AppResult<InitialData> {
        #[cfg(not(test))]
        self.ensure_hub_agent()?;
        let agents = match self.list_agents() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[skills_manager] get_initial_data list_agents failed: {e}");
                Vec::new()
            }
        };
        self.backfill_hub_links(&agents)?;
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
        let result = self.store.toggle_no_full_coverage(title)?;
        // 便携层变更（无文件变化）主动触发一次同步上传。
        self.sync_manager.trigger();
        Ok(result)
    }

    pub fn toggle_no_full_coverage_mcp(&self, title: &str) -> AppResult<bool> {
        // MCP 标记本轮不进同步层，保持本机。
        self.store.toggle_no_full_coverage_mcp(title)
    }

    pub fn set_skill_tags(&self, title: &str, tags: Vec<String>) -> AppResult<Vec<String>> {
        let result = self.store.set_skill_tags(title, tags)?;
        self.sync_manager.trigger();
        Ok(result)
    }

    pub fn set_agent_tags(&self, agent_id: &str, tags: Vec<String>) -> AppResult<Vec<String>> {
        let result = self.store.set_agent_tags(agent_id, tags)?;
        self.sync_manager.trigger();
        Ok(result)
    }

    pub fn list_saved_agents(&self) -> AppResult<Vec<AgentProfile>> {
        self.store.list_agents()
    }

    pub fn list_agents(&self) -> AppResult<Vec<AgentProfile>> {
        #[cfg(not(test))]
        let primary_hub_id = self.ensure_hub_agent()?.id;
        let mut agents = HashMap::new();
        for agent in self.list_saved_agents()? {
            #[cfg(not(test))]
            if (agent.agent_type == AgentType::Universal
                || is_legacy_universal_skills_path(&agent.skills_path)
                || is_universal_skills_path(&agent.skills_path))
                && agent.id != primary_hub_id
            {
                continue;
            }
            agents.insert(agent.id.clone(), agent);
        }
        for agent in self.detect_agents()? {
            #[cfg(not(test))]
            if agent.agent_type == AgentType::Universal && agent.id != primary_hub_id {
                continue;
            }
            agents.entry(agent.id.clone()).or_insert(agent);
        }
        let mut values = agents.into_values().collect::<Vec<_>>();
        for agent in &mut values {
            agent.user_tags = self.store.list_agent_tags(&agent.id)?;
            agent.supports_universal = agent.agent_type == AgentType::Universal
                || is_universal_skills_path(&agent.skills_path);
        }
        values.sort_by(|a, b| a.name.cmp(&b.name).then(a.skills_path.cmp(&b.skills_path)));
        Ok(values)
    }

    pub fn add_agent(&self, mut profile: AgentProfile) -> AppResult<AgentProfile> {
        // 内置类型允许留空名称/目录，使用该类型的默认值，避免用户必须手动填写。
        if profile.name.trim().is_empty() {
            if let Some(name) = profile.agent_type.default_name() {
                profile.name = name.to_string();
            }
        }
        profile.name = profile.name.trim().to_string();
        if profile.skills_path.trim().is_empty() {
            if let Some(path) = default_skills_path(&profile.agent_type) {
                profile.skills_path = path.to_string_lossy().to_string();
            }
        } else {
            profile.skills_path = expand_user_path(&profile.skills_path);
        }
        #[cfg(not(test))]
        if profile.agent_type == AgentType::Universal {
            profile.skills_path = program_skills_path()?.to_string_lossy().to_string();
        }
        profile.supports_universal = profile.agent_type == AgentType::Universal
            || is_universal_skills_path(&profile.skills_path);
        let adapter = adapter_for(&profile);
        adapter.validate(&profile)?;
        self.store.save_agent(&profile)?;
        self.invalidate_installed_cache();
        Ok(profile)
    }

    fn find_hub_agent(agents: &[AgentProfile]) -> Option<&AgentProfile> {
        agents.iter().find(|agent| {
            agent.agent_type == AgentType::Universal
                || is_universal_skills_path(&agent.skills_path)
        })
    }

    /// 确保程序目录中的 Skills 中枢存在；测试环境只使用显式注册的临时中枢。
    pub fn ensure_hub_agent(&self) -> AppResult<AgentProfile> {
        #[cfg(test)]
        {
            return Self::find_hub_agent(&self.list_saved_agents()?)
                .cloned()
                .ok_or_else(|| AppError::Message("测试环境未配置临时 Universal 中枢".to_string()));
        }
        #[cfg(not(test))]
        {
            let skills_path = match &self.demo_hub_path {
                Some(path) => path.clone(),
                None => program_skills_path()?,
            };
            fs::create_dir_all(&skills_path).map_err(|error| AppError::Message(format!(
                "无法写入程序目录中的 Skills 中枢 {}: {}。请将程序安装在可写目录。",
                skills_path.display(), error
            )))?;
            let saved = self.list_saved_agents()?;
            let existing = saved.iter()
                .find(|agent| agent.agent_type == AgentType::Universal
                    && is_universal_skills_path(&agent.skills_path))
                .or_else(|| saved.iter().find(|agent| agent.agent_type == AgentType::Universal))
                .or_else(|| saved.iter().find(|agent| is_legacy_universal_skills_path(&agent.skills_path)));

            let legacy_path = existing
                .map(|agent| PathBuf::from(expand_user_path(&agent.skills_path)))
                .or_else(|| dirs::home_dir().map(|home| home.join(".agents").join("skills")));
            if let Some(ref old_path) = legacy_path {
                if Self::same_path(old_path, &skills_path) { Vec::new() }
                else {
                    let allowed = self.known_managed_legacy_names(old_path)?;
                    migrate_hub_contents(old_path, &skills_path, &allowed)?
                }
            } else {
                Vec::new()
            };

            let mut hub = existing.cloned().unwrap_or_else(|| AgentProfile {
                id: format!("universal:{}", skills_path.display()),
                name: "Skills Manager 中枢".to_string(),
                agent_type: AgentType::Universal,
                skills_path: String::new(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            });
            let new_path = skills_path.to_string_lossy().to_string();
            if hub.skills_path != new_path || hub.agent_type != AgentType::Universal {
                hub.skills_path = new_path;
                hub.agent_type = AgentType::Universal;
                hub.name = "Skills Manager 中枢".to_string();
                hub.supports_universal = true;
                self.store.save_agent(&hub)?;
                self.invalidate_installed_cache();
            }
            self.migrate_legacy_install_records(&skills_path, &saved)?;
            Ok(hub)
        }
    }

    fn same_path(a: &Path, b: &Path) -> bool {
        match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => a == b,
        }
    }

    /// 将技能实体复制进中枢（源已是中枢则跳过），返回中枢内技能路径。
    fn materialize_into_hub(
        &self,
        source_path: &Path,
        source_dir_name: &str,
        skill_label: &str,
        conflict_policy: &ConflictPolicy,
        lock_source: Option<&str>,
        lock_source_url: Option<&str>,
    ) -> AppResult<PathBuf> {
        let hub = self.ensure_hub_agent()?;
        let hub_skills = Path::new(&hub.skills_path);
        fs::create_dir_all(hub_skills)?;
        let hub_target = hub_skills.join(source_dir_name);
        if Self::same_path(source_path, &hub_target) {
            return Ok(hub_target);
        }
        match self.resolve_install_conflict(
            &hub,
            hub_skills,
            source_dir_name,
            skill_label,
            conflict_policy,
        )? {
            None => Ok(hub_target),
            Some((target, _action, _backup)) => {
                copy_dir_all(source_path, &target)?;
                // catalog/install 标记来源类型，便于 lock 生态区分
                let source_type = lock_source
                    .filter(|s| s.starts_with("clawhub") || *s == "import" || s.contains("catalog"))
                    .map(|_| "catalog")
                    .unwrap_or("local");
                let _ = write_skill_lock_entry(
                    source_dir_name,
                    lock_source,
                    Some(source_type),
                    lock_source_url,
                    &target,
                );
                Ok(target)
            }
        }
    }

    /// 从中枢扇出到目标 Agent：专用适配器走其协议；其余优先软链/Junction。
    fn fanout_from_hub(
        &self,
        hub: &AgentProfile,
        hub_skill_path: &Path,
        skill_label: &str,
        source_dir_name: &str,
        target_agent_ids: &[String],
        conflict_policy: &ConflictPolicy,
        skip_agent_id: Option<&str>,
    ) -> AppResult<Vec<InstallResult>> {
        let agents = self.list_agents()?;
        let agent_map: HashMap<_, _> = agents
            .into_iter()
            .map(|agent| (agent.id.clone(), agent))
            .collect();
        let mut results = Vec::new();
        let mut install_records: Vec<InstallRecordInput> = Vec::new();
        let source_fingerprint = hash_dir(hub_skill_path)?;

        for agent_id in target_agent_ids {
            if Some(agent_id.as_str()) == skip_agent_id {
                results.push(InstallResult {
                    agent_id: agent_id.clone(),
                    skill_id: skill_label.to_string(),
                    action: "skipped".to_string(),
                    target_path: hub_skill_path.to_string_lossy().to_string(),
                    backup_path: None,
                    message: format!("{skill_label} 已存在于来源"),
                    distribution_method: None,
                });
                continue;
            }
            let Some(agent) = agent_map.get(agent_id) else {
                results.push(InstallResult {
                    agent_id: agent_id.clone(),
                    skill_id: skill_label.to_string(),
                    action: "error".to_string(),
                    target_path: hub_skill_path.to_string_lossy().to_string(),
                    backup_path: None,
                    message: format!("找不到 Agent: {agent_id}"),
                    distribution_method: None,
                });
                continue;
            };
            if agent.id == hub.id {
                results.push(InstallResult {
                    agent_id: agent.id.clone(),
                    skill_id: skill_label.to_string(),
                    action: "skipped".to_string(),
                    target_path: hub_skill_path.to_string_lossy().to_string(),
                    backup_path: None,
                    message: format!("{skill_label} 已在 Universal 中枢"),
                    distribution_method: None,
                });
                continue;
            }
            let skills_path = Path::new(&agent.skills_path);
            let target = skills_path.join(source_dir_name);
            let skill_key = normalize_title(skill_label);
            let existing = scan_agent_skill_copies(agent)?;
            let other_same_title = existing.iter().any(|copy| {
                normalize_title(&copy.title) == skill_key
                    && Path::new(&copy.skill_path) != target
            });
            let target_exists = fs::symlink_metadata(&target).is_ok();
            let owned = target_exists && self.is_managed_target(agent_id, &skill_key, &target, hub_skill_path)?;
            if other_same_title || (target_exists && !owned) {
                results.push(InstallResult {
                    agent_id: agent.id.clone(), skill_id: skill_label.to_string(),
                    action: "skipped".into(), target_path: target.to_string_lossy().to_string(),
                    backup_path: None,
                    message: format!("{} 已有独立安装的同名 Skill，已保留并跳过", agent.name),
                    distribution_method: None,
                });
                continue;
            }
            if owned && is_symlink_path(&target) && Self::same_path(&target, hub_skill_path) {
                self.store.upsert_managed_target(ManagedTargetRecord {
                    skill_key: skill_key.clone(), dir_name: source_dir_name.to_string(),
                    agent_id: agent.id.clone(), target_path: target.to_string_lossy().to_string(),
                    method: "link".into(), fingerprint: source_fingerprint.clone(),
                })?;
                results.push(InstallResult {
                    agent_id: agent.id.clone(), skill_id: skill_label.to_string(),
                    action: "linked".into(), target_path: target.to_string_lossy().to_string(),
                    backup_path: None, message: format!("{} 已链接中枢 Skill", agent.name),
                    distribution_method: Some("link".into()),
                });
                continue;
            }
            let _ = fs::create_dir_all(skills_path);
            match self.install_skill_into_agent(
                agent,
                hub_skill_path,
                source_dir_name,
                skill_label,
                conflict_policy,
                true,
                true,
            ) {
                Ok(result) => {
                    if matches!(result.action.as_str(), "installed" | "updated" | "renamed") {
                        let method = result.distribution_method.as_deref().unwrap_or("copy");
                        self.store.upsert_managed_target(ManagedTargetRecord {
                            skill_key: skill_key.clone(), dir_name: source_dir_name.to_string(),
                            agent_id: agent.id.clone(), target_path: result.target_path.clone(),
                            method: method.into(), fingerprint: source_fingerprint.clone(),
                        })?;
                        install_records.push(InstallRecordInput {
                            agent_id: agent.id.clone(),
                            skill_id: skill_label.to_string(),
                            fingerprint: source_fingerprint.clone(),
                            target_path: result.target_path.clone(),
                            action: result.action.clone(),
                            backup_path: result.backup_path.clone(),
                        });
                    }
                    results.push(result);
                }
                Err(error) => results.push(InstallResult {
                    agent_id: agent.id.clone(),
                    skill_id: skill_label.to_string(),
                    action: "error".to_string(),
                    target_path: skills_path
                        .join(source_dir_name)
                        .to_string_lossy()
                        .to_string(),
                    backup_path: None,
                    message: format!("同步 {skill_label} 到 {} 失败: {}", agent.name, error),
                    distribution_method: None,
                }),
            }
        }

        if !install_records.is_empty() {
            self.store.record_installs(&install_records)?;
        }
        self.invalidate_installed_cache();
        Ok(results)
    }

    fn known_managed_legacy_names(&self, old_root: &Path) -> AppResult<HashSet<String>> {
        let mut names = HashSet::new();
        for (_, _, target, fingerprint) in self.store.list_install_records()? {
            let path = Path::new(&target);
            if path.parent().map(|parent| Self::same_path(parent, old_root)).unwrap_or(false)
                && path.is_dir() && hash_dir(path).ok().as_deref() == Some(fingerprint.as_str()) {
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) { names.insert(name.to_string()); }
            }
        }
        Ok(names)
    }

    /// 只迁移旧安装记录且内容未被改动的 Agent 副本。原目录保留，登记为本机托管副本。
    fn migrate_legacy_install_records(&self, hub_root: &Path, agents: &[AgentProfile]) -> AppResult<()> {
        let known = self.store.list_managed_targets()?;
        for (agent_id, title, target_path, fingerprint) in self.store.list_install_records()? {
            let Some(agent) = agents.iter().find(|agent| agent.id == agent_id && agent.agent_type != AgentType::Universal) else { continue; };
            let target = Path::new(&target_path);
            if !target.parent().map(|parent| Self::same_path(parent, Path::new(&agent.skills_path))).unwrap_or(false)
                || !target.is_dir() || is_symlink_path(target)
                || hash_dir(target).ok().as_deref() != Some(fingerprint.as_str()) { continue; }
            let Some(dir_name) = target.file_name().and_then(|name| name.to_str()) else { continue; };
            let key = normalize_title(&title);
            if known.iter().any(|item| item.agent_id == agent_id && item.skill_key == key) { continue; }
            let hub_skill = hub_root.join(dir_name);
            if fs::symlink_metadata(&hub_skill).is_err() {
                let staging = hub_root.join(format!(".migration-{}", uuid::Uuid::new_v4()));
                copy_dir_all(target, &staging)?;
                fs::rename(&staging, &hub_skill)?;
            }
            if hub_skill.is_dir() && hash_dir(&hub_skill).ok().as_deref() == Some(fingerprint.as_str()) {
                self.store.upsert_managed_target(ManagedTargetRecord {
                    skill_key: key, dir_name: dir_name.to_string(), agent_id,
                    target_path, method: "copy".into(), fingerprint,
                })?;
            }
        }
        Ok(())
    }

    fn backfill_hub_links(&self, agents: &[AgentProfile]) -> AppResult<()> {
        let Some(hub) = Self::find_hub_agent(agents) else { return Ok(()); };
        let hub_root = Path::new(&hub.skills_path);
        if !hub_root.is_dir() { return Ok(()); }
        let known = self.store.list_managed_targets()?;
        for entry in fs::read_dir(hub_root)? {
            let entry = entry?;
            let skill = entry.path();
            if !skill.is_dir() || entry.file_name().to_string_lossy().starts_with('.') { continue; }
            let Some(dir_name) = entry.file_name().to_str().map(str::to_string) else { continue; };
            let (title, _, _, _) = read_agent_skill_info(&skill, false);
            let key = normalize_title(if title.is_empty() { &dir_name } else { &title });
            let fingerprint = hash_dir(&skill)?;
            for agent in agents.iter().filter(|agent| agent.id != hub.id) {
                let target = Path::new(&agent.skills_path).join(&dir_name);
                if known.iter().any(|item| item.agent_id == agent.id && item.skill_key == key) { continue; }
                if is_symlink_path(&target) && Self::same_path(&target, &skill) {
                    self.store.upsert_managed_target(ManagedTargetRecord {
                        skill_key: key.clone(), dir_name: dir_name.clone(), agent_id: agent.id.clone(),
                        target_path: target.to_string_lossy().to_string(), method: "link".into(),
                        fingerprint: fingerprint.clone(),
                    })?;
                }
            }
        }
        Ok(())
    }

    fn is_managed_target(&self, agent_id: &str, skill_key: &str, target: &Path, hub_skill: &Path) -> AppResult<bool> {
        if is_symlink_path(target) {
            return Ok(Self::same_path(target, hub_skill));
        }
        let record = self.store.list_managed_targets()?.into_iter().find(|item| {
            item.agent_id == agent_id && item.skill_key == skill_key
                && Self::same_path(Path::new(&item.target_path), target)
        });
        Ok(record.map(|item| item.method != "link" && hash_dir(target).ok().as_deref() == Some(item.fingerprint.as_str())).unwrap_or(false))
    }

    /// 将某个来源目录安装到单个 Agent。
    /// `link_generic=true` 时普通 Agent 优先软链（中枢 fanout）。
    /// `from_hub` 仅影响提示文案。三条安装路径（安装/同步/导入）共用，避免逻辑分叉。
    fn install_skill_into_agent(
        &self,
        agent: &AgentProfile,
        source_path: &Path,
        source_dir_name: &str,
        skill_label: &str,
        conflict_policy: &ConflictPolicy,
        link_generic: bool,
        from_hub: bool,
    ) -> AppResult<InstallResult> {
        let skills_path = Path::new(&agent.skills_path);
        fs::create_dir_all(skills_path)?;
        let Some((target, action, backup_path)) = self.resolve_install_conflict(
            agent,
            skills_path,
            source_dir_name,
            skill_label,
            conflict_policy,
        )? else {
            return Ok(InstallResult {
                agent_id: agent.id.clone(),
                skill_id: skill_label.to_string(),
                action: "skipped".to_string(),
                target_path: skills_path
                    .join(source_dir_name)
                    .to_string_lossy()
                    .to_string(),
                backup_path: None,
                message: format!("已跳过 {skill_label}"),
                distribution_method: None,
            });
        };

        let distribution_method;
        if agent.agent_type == AgentType::CherryStudio {
            let cs = CherryStudioAdapter::new().ok_or_else(|| {
                AppError::Message(
                    "未找到 Cherry Studio 安装目录（%APPDATA%\\CherryStudio 缺失），无法安装。"
                        .to_string(),
                )
            })?;
            cs.install_skill(source_path, source_dir_name)?;
            distribution_method = "special";
        } else if agent.agent_type == AgentType::ClaudeCowork {
            copy_dir_all(source_path, &target)?;
            register_claude_cowork_skill(agent, source_dir_name, &target)?;
            distribution_method = "special";
        } else if link_generic {
            distribution_method = symlink_or_copy_dir(source_path, &target)?;
        } else {
            copy_dir_all(source_path, &target)?;
            distribution_method = "copy";
        }

        let sync_msg = if from_hub {
            match action.as_str() {
                "updated" => {
                    if distribution_method == "symlink" || distribution_method == "junction" {
                        format!("已通过{}从中枢更新 {skill_label} 到 {}", if distribution_method == "junction" { "Junction" } else { "软链接" }, agent.name)
                    } else {
                        format!("已更新中枢 Skill 的托管副本 {skill_label} 到 {}", agent.name)
                    }
                }
                "renamed" => format!("已另存副本 {skill_label} 到 {}", agent.name),
                _ => {
                    if distribution_method == "symlink" || distribution_method == "junction" {
                        format!("已通过{}从中枢同步 {skill_label} 到 {}", if distribution_method == "junction" { "Junction" } else { "软链接" }, agent.name)
                    } else {
                        format!("{skill_label} 已作为中枢托管副本复制到 {}", agent.name)
                    }
                }
            }
        } else {
            match action.as_str() {
                "updated" => format!("已更新 {skill_label} 到 {}", agent.name),
                "renamed" => format!("已另存副本 {skill_label} 到 {}", agent.name),
                _ => format!("已复制 {skill_label} 到 {}", agent.name),
            }
        };

        Ok(InstallResult {
            agent_id: agent.id.clone(),
            skill_id: skill_label.to_string(),
            action,
            target_path: target.to_string_lossy().to_string(),
            backup_path,
            message: sync_msg,
            distribution_method: Some(distribution_method.into()),
        })
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
        to_hub: bool,
    ) -> AppResult<Vec<InstallResult>> {
        if !to_hub { return Err(AppError::Message("Skills Manager 添加的 Skill 必须先保存到中枢；已取消仅本机安装。".into())); }
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
        let agents = self.list_agents()?;
        for agent_id in &target_agent_ids {
            if !agents.iter().any(|agent| &agent.id == agent_id) {
                return Err(AppError::Message(format!("找不到 Agent: {}", agent_id)));
            }
        }

        self.materialize_into_hub(
            source_path,
            source_dir_name,
            &skill.name,
            &conflict_policy,
            Some(&skill.source_id),
            None,
        )?;
        self.set_hub_skill_targets(&skill.name, &target_agent_ids)
    }

    pub fn scan_agent_skills(&self) -> AppResult<Vec<GroupedSkill>> {
        let agents = self.list_agents()?;
        let lock_map = load_skill_lock_map();
        let mut copies = Vec::new();
        for agent in &agents {
            copies.extend(scan_agent_skill_copies_with_lock(agent, &lock_map)?);
        }
        if let Some(hub) = Self::find_hub_agent(&agents) {
            let managed = self.store.list_managed_targets()?;
            for copy in &mut copies {
                if copy.agent_id == hub.id { copy.managed_by_hub = true; continue; }
                let key = normalize_title(&copy.title);
                if let Some(record) = managed.iter().find(|record| record.agent_id == copy.agent_id
                    && record.skill_key == key && Path::new(&record.target_path) == Path::new(&copy.skill_path)) {
                    let hub_skill = Path::new(&hub.skills_path).join(&record.dir_name);
                    copy.managed_by_hub = self.is_managed_target(&copy.agent_id, &key, Path::new(&copy.skill_path), &hub_skill)?;
                } else if copy.is_symlink {
                    let dir_name = Path::new(&copy.skill_path).file_name().and_then(|name| name.to_str()).unwrap_or("");
                    copy.managed_by_hub = Self::link_points_to(Path::new(&copy.skill_path), &Path::new(&hub.skills_path).join(dir_name));
                }
            }
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
        to_hub: bool,
    ) -> AppResult<Vec<InstallResult>> {
        if !to_hub { return Err(AppError::Message("Skills Manager 同步的 Skill 必须先保存到中枢；已取消仅本机安装。".into())); }
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
        for agent_id in &target_agent_ids {
            if !agents.iter().any(|agent| &agent.id == agent_id) {
                return Err(AppError::Message(format!("找不到 Agent: {}", agent_id)));
            }
        }

        let source_path = Path::new(&source.skill_path);
        let source_dir_name = source_path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::Message("来源 skill 路径无效".to_string()))?;

        // 强制先入中枢，再从中枢扇出，禁止 Agent→Agent 直接软链
        self.materialize_into_hub(
            source_path,
            source_dir_name,
            title,
            &conflict_policy,
            Some(&source.agent_id),
            source.source_url.as_deref(),
        )?;
        self.set_hub_skill_targets(title, &target_agent_ids)
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
        if self.store.list_managed_targets()?.iter().any(|record| {
            record.agent_id == agent_id && record.skill_key == normalize_title(skill_id)
                && Path::new(&record.target_path) == target_path
        }) {
            self.store.remove_managed_target(&normalize_title(skill_id), agent_id)?;
        }
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
        to_hub: bool,
    ) -> AppResult<ImportSkillResult> {
        if !to_hub { return Err(AppError::Message("Skills Manager 导入的 Skill 必须先保存到中枢；已取消仅本机安装。".into())); }
        if files.is_empty() {
            return Err(AppError::Message("上传内容为空".to_string()));
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
        for agent_id in target_agent_ids {
            if !agents.iter().any(|a| &a.id == agent_id) {
                return Err(AppError::Message(format!("找不到 Agent: {}", agent_id)));
            }
        }

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

            // 先入中枢再扇出
            let hub_skill_path = match self.materialize_into_hub(
                source,
                skill_dir_name,
                &skill.manifest.id,
                &conflict_policy,
                Some("import"),
                None,
            ) {
                Ok(path) => path,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            let hub = match self.ensure_hub_agent() {
                Ok(hub) => hub,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            let results = self.fanout_from_hub(
                &hub,
                &hub_skill_path,
                &skill.manifest.id,
                skill_dir_name,
                &target_agent_ids,
                &conflict_policy,
                None,
            )?;
            if target_agent_ids.is_empty() { imported += 1; }
            for result in results {
                match result.action.as_str() {
                    "error" => skipped += 1,
                        "linked" | "skipped" => skipped += 1,
                    _ => imported += 1,
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

    // ── 同步编排 ──────────────────────────────────────────────────────

    pub fn sync_get_config(&self) -> AppResult<SyncConfig> {
        self.store.sync_config()
    }

    /// 保存同步配置；`secret_access_key` / `encrypt_password` 为 `Some` 时写入钥匙串，
    /// 传空字符串表示清除。密钥不落 `state.json`。
    pub fn sync_set_config(
        &self,
        config: SyncConfig,
        secret_access_key: Option<String>,
        encrypt_password: Option<String>,
    ) -> AppResult<SyncConfig> {
        if let Some(value) = secret_access_key {
            if value.trim().is_empty() {
                self.secrets.delete(SECRET_ACCESS_KEY)?;
            } else {
                self.secrets.set(SECRET_ACCESS_KEY, value.trim())?;
            }
        }
        if let Some(value) = encrypt_password {
            if value.is_empty() {
                self.secrets.delete(ENCRYPT_PASSWORD)?;
            } else {
                self.secrets.set(ENCRYPT_PASSWORD, &value)?;
            }
        }
        self.store.ensure_device_id()?;
        self.store.set_sync_config(&config)?;
        if config.enabled {
            self.sync_manager.start(self.clone(), config.poll_secs);
            self.start_sync_watcher();
        } else {
            self.sync_manager.stop();
            self.stop_sync_watcher();
        }
        Ok(config)
    }

    /// 启动中枢文件监听（幂等）。测试环境不启动，避免触发真实目录与并行干扰。
    fn start_sync_watcher(&self) {
        #[cfg(test)]
        {
            return;
        }
        #[cfg(not(test))]
        {
            if self
                .sync_watcher
                .lock()
                .map(|guard| guard.is_some())
                .unwrap_or(false)
            {
                return;
            }
            let hub = match self.ensure_hub_agent() {
                Ok(hub) => hub,
                Err(_) => return,
            };
            let trigger_manager = self.sync_manager.clone();
            let skip_manager = self.sync_manager.clone();
            match crate::sync::spawn_hub_watcher(
                PathBuf::from(&hub.skills_path),
                move || trigger_manager.trigger(),
                move || skip_manager.is_suppressed(),
            ) {
                Ok(watcher) => {
                    if let Ok(mut guard) = self.sync_watcher.lock() {
                        *guard = Some(watcher);
                    }
                }
                Err(error) => {
                    eprintln!("[skills_manager] 启动同步文件监听失败: {error}");
                }
            }
        }
    }

    fn stop_sync_watcher(&self) {
        if let Ok(mut guard) = self.sync_watcher.lock() {
            *guard = None;
        }
    }

    pub fn sync_status(&self) -> AppResult<SyncStatus> {
        let config = self.store.sync_config()?;
        let has_secret = self
            .secrets
            .get(SECRET_ACCESS_KEY)?
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        let has_password = self
            .secrets
            .get(ENCRYPT_PASSWORD)?
            .map(|value| !value.is_empty())
            .unwrap_or(false);
        let configured = if is_local_endpoint(&config.endpoint) {
            // 本地目录传输：不要求 S3 AK/SK/bucket；若启用加密则仍需口令。
            !config.endpoint.trim().is_empty() && (!config.encrypt || has_password)
        } else {
            !config.endpoint.trim().is_empty()
                && !config.bucket.trim().is_empty()
                && !config.access_key_id.trim().is_empty()
                && has_secret
                && (!config.encrypt || has_password)
        };
        Ok(SyncStatus {
            configured,
            enabled: config.enabled,
            running: self.sync_manager.is_running(),
            last_run_at: self.store.sync_last_run_at()?,
            last_error: self.store.sync_last_error()?,
            pending_conflicts: self.store.sync_conflicts()?.len(),
            device_id: self.store.ensure_device_id()?,
            device_name: self.store.device_name()?,
        })
    }

    pub fn sync_list_conflicts(&self) -> AppResult<Vec<crate::models::SyncConflict>> {
        self.store.sync_conflicts()
    }

    fn build_transport(&self, config: &SyncConfig) -> AppResult<Box<dyn SyncTransport>> {
        build_transport_from(config, self.secrets.as_ref())
    }

    fn build_crypto(
        &self,
        transport: &dyn SyncTransport,
        config: &SyncConfig,
    ) -> AppResult<Option<Crypto>> {
        if !config.encrypt {
            return Ok(None);
        }
        let password = self
            .secrets
            .get(ENCRYPT_PASSWORD)?
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::Message("未设置同步加密口令。".to_string()))?;
        let salt = ensure_kdf_salt(transport)?;
        Ok(Some(Crypto::from_password(&password, &salt)?))
    }

    pub fn sync_test_connection(&self, config: SyncConfig) -> AppResult<String> {
        let transport = self.build_transport(&config)?;
        transport.test_connection()?;
        if is_local_endpoint(&config.endpoint) {
            let root = local_root_from_endpoint(&config.endpoint)
                .map(|path| {
                    if config.bucket.trim().is_empty() {
                        path
                    } else {
                        path.join(config.bucket.trim())
                    }
                })
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| config.endpoint.clone());
            Ok(format!("本地目录可用：{root}"))
        } else {
            Ok(format!("连接成功：{} / {}", config.endpoint, config.bucket))
        }
    }

    /// 立即同步：先发布本机，再拉取远端并落中枢，最后 reconcile fanout。
    pub fn sync_now(&self) -> AppResult<SyncStatus> {
        let _guard = self.sync_manager.lock();
        let config = self.store.sync_config()?;
        ensure_sync_transport_ready(&config)?;
        let transport = self.build_transport(&config)?;
        self.sync_run_with(transport.as_ref(), &config)
    }

    /// 后台轮询调用：未启用 / 未配置时静默跳过；失败仅记录 `sync_last_error`。
    pub fn sync_auto_once(&self) -> AppResult<()> {
        let _guard = self.sync_manager.lock();
        let config = self.store.sync_config()?;
        if !config.enabled || ensure_sync_transport_ready(&config).is_err() {
            return Ok(());
        }
        let transport = match self.build_transport(&config) {
            Ok(transport) => transport,
            Err(error) => {
                self.store.set_sync_last_error(Some(&error.to_string()))?;
                return Err(error);
            }
        };
        let _ = self.sync_run_with(transport.as_ref(), &config);
        Ok(())
    }

    /// 用给定传输执行一次同步（生产用 S3/本地目录；测试可注入传输）。
    pub(crate) fn sync_run_with(
        &self,
        transport: &dyn SyncTransport,
        config: &SyncConfig,
    ) -> AppResult<SyncStatus> {
        // 抑制 watcher：本机落中枢/写清单不应再触发一轮自同步。
        let _op_guard = self.sync_manager.operation_guard();
        let hub = self.ensure_hub_agent()?;
        self.backfill_hub_links(&self.list_agents()?)?;
        let crypto = self.build_crypto(transport, config)?;
        let result = (|| -> AppResult<()> {
            let runtime = SyncRuntime::new(
                self.store.as_ref(),
                transport,
                crypto.as_ref(),
                PathBuf::from(&hub.skills_path),
            )?;
            runtime.publish()?;
            let pull = runtime.pull()?;
            for dir in &pull.new_dirs {
                let path = Path::new(&hub.skills_path).join(dir);
                let (title, _, _, _) = read_agent_skill_info(&path, false);
                let title = if title.trim().is_empty() { dir.clone() } else { title };
                self.store.add_pending_hub_skill(PendingHubSkill {
                    skill_key: normalize_title(&title), title, dir_name: dir.clone(), notified: false, message: None,
                })?;
            }
            self.reconcile_from_hub(&pull.applied_dirs)?;
            self.remove_deleted_managed(&pull.deleted_dirs, Path::new(&hub.skills_path))?;
            Ok(())
        })();

        self.store
            .set_sync_last_run_at(Some(chrono::Utc::now().to_rfc3339()))?;
        match result {
            Ok(()) => {
                self.store.set_sync_last_error(None)?;
                self.sync_status()
            }
            Err(error) => {
                self.store.set_sync_last_error(Some(&error.to_string()))?;
                Err(error)
            }
        }
    }

    /// 冲突解决：local=保留本机，remote=用远端覆盖，rename=远端改名保存。
    pub fn sync_resolve_conflict(
        &self,
        skill_id: &str,
        choice: SyncConflictChoice,
    ) -> AppResult<SyncStatus> {
        let _guard = self.sync_manager.lock();
        let conflicts = self.store.sync_conflicts()?;
        let conflict = conflicts
            .iter()
            .find(|conflict| conflict.skill_id == skill_id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("找不到冲突: {skill_id}")))?;

        let hub = self.ensure_hub_agent()?;
        let config = self.store.sync_config()?;
        let transport = self.build_transport(&config)?;
        let crypto = self.build_crypto(transport.as_ref(), &config)?;
        let runtime = SyncRuntime::new(
            self.store.as_ref(),
            transport.as_ref(),
            crypto.as_ref(),
            PathBuf::from(&hub.skills_path),
        )?;

        match choice {
            SyncConflictChoice::Local => {
                runtime.update_baseline(skill_id, conflict.local_hash.clone())?;
            }
            SyncConflictChoice::Remote => {
                if let Some(hash) = &conflict.remote_hash {
                    runtime.apply_remote_blob(&conflict.dir_name, hash)?;
                    runtime.update_baseline(skill_id, Some(hash.clone()))?;
                    let _ = self.reconcile_from_hub(&[conflict.dir_name.clone()]);
                }
            }
            SyncConflictChoice::Rename => {
                let renamed = format!(
                    "{}-remote-{}",
                    conflict.dir_name,
                    chrono::Utc::now().format("%Y%m%d%H%M%S%3f")
                );
                if let Some(hash) = &conflict.remote_hash {
                    runtime.apply_remote_blob(&renamed, hash)?;
                }
                runtime.update_baseline(skill_id, conflict.local_hash.clone())?;
            }
        }

        let remaining: Vec<_> = conflicts
            .into_iter()
            .filter(|current| current.skill_id != skill_id)
            .collect();
        self.store.set_sync_conflicts(remaining)?;
        self.sync_status()
    }

    /// 清理未被任何设备清单引用的 blob，返回删除数量。
    pub fn sync_gc(&self) -> AppResult<usize> {
        let _guard = self.sync_manager.lock();
        let config = self.store.sync_config()?;
        ensure_sync_transport_ready(&config)?;
        let transport = self.build_transport(&config)?;
        let crypto = self.build_crypto(transport.as_ref(), &config)?;
        let hub = self.ensure_hub_agent()?;
        let runtime = SyncRuntime::new(
            self.store.as_ref(),
            transport.as_ref(),
            crypto.as_ref(),
            PathBuf::from(&hub.skills_path),
        )?;
        let outcome = runtime.gc()?;
        Ok(outcome.removed)
    }

    /// 拉取落中枢后，把新落地的 skill fanout 到各 agent（逐 skill 失败隔离）。
    fn reconcile_from_hub(&self, dir_names: &[String]) -> AppResult<()> {
        if dir_names.is_empty() {
            return Ok(());
        }
        let agents = self.list_agents()?;
        let Some(hub) = Self::find_hub_agent(&agents).cloned() else {
            return Ok(());
        };
        let managed = self.store.list_managed_targets()?;
        let hub_skills = Path::new(&hub.skills_path);
        for dir in dir_names {
            let path = hub_skills.join(dir);
            if !path.exists() {
                continue;
            }
            let (title, _version, _description, _readme) = read_agent_skill_info(&path, false);
            let label = if title.trim().is_empty() {
                dir.clone()
            } else {
                title
            };
            let key = normalize_title(&label);
            let target_ids: Vec<String> = managed.iter()
                .filter(|item| item.skill_key == key && agents.iter().any(|agent| agent.id == item.agent_id))
                .map(|item| item.agent_id.clone()).collect();
            if target_ids.is_empty() { continue; }
            match self.fanout_from_hub(
                &hub,
                &path,
                &label,
                dir,
                &target_ids,
                &ConflictPolicy::BackupOverwrite,
                None,
            ) {
                Ok(results) => {
                    if let Some(problem) = results.iter().find(|result| matches!(result.action.as_str(), "skipped" | "error")) {
                        self.store.add_pending_hub_skill(PendingHubSkill {
                            skill_key: key, title: label, dir_name: dir.clone(), notified: false,
                            message: Some(format!("中枢 Skill 更新后，{}。请检查该 Agent 的副本。", problem.message)),
                        })?;
                    }
                }
                Err(error) => self.store.add_pending_hub_skill(PendingHubSkill {
                    skill_key: key, title: label, dir_name: dir.clone(), notified: false,
                    message: Some(format!("中枢 Skill 分发失败：{error}")),
                })?,
            }
        }
        Ok(())
    }

    /// 更新本机选中的分发目标。未选目标只移除经核实的管理器安装。
    pub fn set_hub_skill_targets(&self, title: &str, target_agent_ids: &[String]) -> AppResult<Vec<InstallResult>> {
        let key = normalize_title(title);
        let hub = self.ensure_hub_agent()?;
        let hub_skill = fs::read_dir(&hub.skills_path)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.is_dir() && {
                let (found, _, _, _) = read_agent_skill_info(path, false);
                normalize_title(if found.is_empty() { path.file_name().and_then(|name| name.to_str()).unwrap_or("") } else { &found }) == key
            })
            .ok_or_else(|| AppError::Message(format!("中枢不存在 Skill: {title}")))?;
        let dir_name = hub_skill.file_name().and_then(|name| name.to_str()).unwrap_or("").to_string();
        let agents = self.list_agents()?;
        for id in target_agent_ids {
            if !agents.iter().any(|agent| &agent.id == id && agent.id != hub.id) {
                return Err(AppError::Message(format!("找不到可分发的 Agent: {id}")));
            }
        }
        let mut results = Vec::new();
        for record in self.store.list_managed_targets()?.into_iter()
            .filter(|record| record.skill_key == key && !target_agent_ids.contains(&record.agent_id)) {
            let path = Path::new(&record.target_path);
            let safe = if matches!(record.method.as_str(), "link" | "symlink" | "junction") { Self::link_points_to(path, &hub_skill) }
                else { path.is_dir() && hash_dir(path).ok().as_deref() == Some(record.fingerprint.as_str()) };
            if safe {
                if let Some(agent) = agents.iter().find(|agent| agent.id == record.agent_id) {
                    if agent.agent_type == AgentType::CherryStudio {
                        if let Some(adapter) = CherryStudioAdapter::new() { adapter.uninstall_skill(&record.dir_name)?; }
                    } else if agent.agent_type == AgentType::ClaudeCowork {
                        adapter_for(agent).uninstall(&record.dir_name, agent)?;
                    } else { remove_dir_or_symlink(path)?; }
                } else { remove_dir_or_symlink(path)?; }
                self.store.remove_managed_target(&key, &record.agent_id)?;
                results.push(InstallResult { agent_id: record.agent_id, skill_id: title.into(), action: "unlinked".into(),
                    target_path: record.target_path, backup_path: None, message: "已移除管理器分发的链接或副本".into(), distribution_method: Some(record.method) });
            } else {
                self.store.remove_managed_target(&key, &record.agent_id)?;
                results.push(InstallResult { agent_id: record.agent_id, skill_id: title.into(), action: "skipped".into(),
                    target_path: record.target_path, backup_path: None, message: "目标已被改动，保留原目录并停止自动分发".into(), distribution_method: Some(record.method) });
            }
        }
        results.extend(self.fanout_from_hub(&hub, &hub_skill, title, &dir_name, target_agent_ids, &ConflictPolicy::BackupOverwrite, None)?);
        if let Some(problem) = results.iter().find(|result| matches!(result.action.as_str(), "skipped" | "error")) {
            self.store.add_pending_hub_skill(PendingHubSkill {
                skill_key: key.clone(), title: title.into(), dir_name, notified: false,
                message: Some(problem.message.clone()),
            })?;
        } else {
            self.store.dismiss_pending_hub_skill(&key)?;
        }
        Ok(results)
    }

    pub fn list_pending_hub_skills(&self) -> AppResult<Vec<PendingHubSkill>> { self.store.list_pending_hub_skills() }
    pub fn acknowledge_pending_hub_skills(&self, keys: &[String]) -> AppResult<()> { self.store.acknowledge_pending_hub_skills(keys) }
    pub fn dismiss_pending_hub_skill(&self, key: &str) -> AppResult<()> { self.store.dismiss_pending_hub_skill(key) }

    fn remove_deleted_managed(&self, deleted: &[(String, String)], hub_root: &Path) -> AppResult<()> {
        let records = self.store.list_managed_targets()?;
        for (key, dir_name) in deleted {
            let expected = hub_root.join(dir_name);
            for record in records.iter().filter(|record| &record.skill_key == key) {
                let target = Path::new(&record.target_path);
                let safe = if matches!(record.method.as_str(), "link" | "symlink" | "junction") {
                    Self::link_points_to(target, &expected)
                } else {
                    target.is_dir() && hash_dir(target).ok().as_deref() == Some(record.fingerprint.as_str())
                };
                if safe {
                    remove_dir_or_symlink(target)?;
                    self.store.remove_managed_target(key, &record.agent_id)?;
                } else {
                    self.store.remove_managed_target(key, &record.agent_id)?;
                    self.store.add_pending_hub_skill(PendingHubSkill {
                        skill_key: key.clone(), title: key.clone(), dir_name: dir_name.clone(), notified: false,
                        message: Some(format!("中枢 Skill 已删除，但 Agent 中的副本 {} 已被改动，已保留供手动处理。", target.display())),
                    })?;
                }
            }
            if !self.store.list_pending_hub_skills()?.iter().any(|item| item.skill_key == *key && item.message.is_some()) {
                self.store.dismiss_pending_hub_skill(key)?;
            }
        }
        Ok(())
    }

    fn link_points_to(link: &Path, expected: &Path) -> bool {
        if !is_symlink_path(link) { return false; }
        let Some(raw) = read_symlink_target(link) else { return false; };
        let resolved = if raw.is_relative() { link.parent().unwrap_or(Path::new("")).join(raw) } else { raw };
        let normalize = |path: &Path| path.to_string_lossy().replace('\\', "/")
            .trim_start_matches("//?/").trim_end_matches('/').to_lowercase();
        normalize(&resolved) == normalize(expected)
    }

    // ── 远程源码与外部 Agent 集成 ───────────────────────────────────────────────

    pub(crate) fn remote_cache_root(&self) -> PathBuf {
        self.store.data_dir().join("remote_cache")
    }

    /// 探测远程 Git 仓库或本地目录的内容（Skills 与 MCP 服务）
    pub fn inspect_remote_source(&self, url: &str) -> AppResult<RemoteSourceInspection> {
        let trimmed = url.trim();
        let local_path = Path::new(trimmed);

        let (target_dir, repo_name, branch, subpath, _is_local) = if local_path.is_dir() {
            let dir_name = local_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("local_source")
                .to_string();
            (local_path.to_path_buf(), dir_name, None, None, true)
        } else {
            let parsed = crate::remote_source::parse_git_url(trimmed)?;
            let fetcher = crate::remote_source::RemoteFetcher::new(self.remote_cache_root());
            let fetched_dir = fetcher.fetch(&parsed)?;
            (
                fetched_dir,
                parsed.repo.clone(),
                parsed.branch.clone(),
                parsed.subpath.clone(),
                false,
            )
        };

        let (detected_skills, detected_mcp, detected_type) =
            crate::remote_source::inspect_directory(&target_dir, subpath.as_deref())?;

        let agents = self.list_agents()?;
        let mut available_agents = Vec::new();
        let mut recommended_agent_ids = Vec::new();

        for agent in &agents {
            let supports_mcp = matches!(
                agent.agent_type,
                crate::models::AgentType::Codex
                    | crate::models::AgentType::ClaudeCode
                    | crate::models::AgentType::OpenCode
                    | crate::models::AgentType::Trae
            ) || (agent.agent_type == crate::models::AgentType::Custom
                && agent
                    .adapter_config
                    .as_ref()
                    .and_then(|c| c.get("mcpFormat"))
                    .is_some());

            let is_hub = agent.agent_type == crate::models::AgentType::Universal
                || crate::util::is_universal_skills_path(&agent.skills_path);
            available_agents.push(crate::models::AgentChoice {
                id: agent.id.clone(),
                name: agent.name.clone(),
                agent_type: agent.agent_type.clone(),
                skills_dir: agent.skills_path.clone(),
                is_hub,
                supports_universal: agent.supports_universal,
                supports_mcp,
            });
        }

        if let Ok(hub) = self.ensure_hub_agent() {
            recommended_agent_ids.push(hub.id);
        } else if let Some(first) = available_agents.first() {
            recommended_agent_ids.push(first.id.clone());
        }

        Ok(RemoteSourceInspection {
            url: trimmed.to_string(),
            repo_name,
            branch,
            subpath,
            detected_type,
            skills: detected_skills,
            mcp_servers: detected_mcp,
            available_agents,
            recommended_agent_ids,
        })
    }

    /// 从远程或本地目录安装 Skill（可单技能或多技能集合）
    pub fn install_remote_source(
        &self,
        options: crate::models::RemoteInstallOptions,
    ) -> AppResult<crate::models::RemoteInstallResult> {
        if options.to_hub == Some(false) { return Err(AppError::Message("Skills Manager 添加的 Skill 必须先保存到中枢；已取消仅本机安装。".into())); }

        let trimmed = options.url.trim();
        let local_path = Path::new(trimmed);

        let (target_dir, subpath) = if local_path.is_dir() {
            (local_path.to_path_buf(), None)
        } else {
            let parsed = crate::remote_source::parse_git_url(trimmed)?;
            let fetcher = crate::remote_source::RemoteFetcher::new(self.remote_cache_root());
            let fetched_dir = fetcher.fetch(&parsed)?;
            (fetched_dir, parsed.subpath)
        };

        let (detected_skills, _, _) =
            crate::remote_source::inspect_directory(&target_dir, subpath.as_deref())?;

        if detected_skills.is_empty() {
            return Err(AppError::Message(
                "在指定的仓库或目录中未发现可识别的 Skill (未找到 SKILL.md 或 skill.json/yaml)".into(),
            ));
        }

        let mut results = Vec::new();
        let mut installed_count = 0;

        let base_dir = if let Some(sub) = &subpath {
            target_dir.join(sub)
        } else {
            target_dir.clone()
        };

        for skill_info in &detected_skills {
            if !options.selected_skills.is_empty()
                && !options.selected_skills.contains(&skill_info.name)
                && !options.selected_skills.contains(&skill_info.title)
                && !options.selected_skills.contains(&skill_info.relative_path)
            {
                continue;
            }

            let skill_source_dir = if skill_info.relative_path.is_empty() {
                base_dir.clone()
            } else {
                base_dir.join(&skill_info.relative_path)
            };

            let skill_dir_name = skill_source_dir
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or(&skill_info.name);

            // 先入中枢再扇出
            let hub_skill_path = self.materialize_into_hub(
                &skill_source_dir,
                skill_dir_name,
                &skill_info.name,
                &options.conflict_policy,
                Some("git"),
                None,
            )?;

            // 记录来源到 .skill-lock.json
            let _ = crate::skill_scan::write_skill_lock_entry(
                skill_dir_name,
                Some("git"),
                Some("git"),
                Some(&options.url),
                &hub_skill_path,
            );

            let hub = self.ensure_hub_agent()?;
            let fanout_res = self.fanout_from_hub(
                &hub,
                &hub_skill_path,
                &skill_info.name,
                skill_dir_name,
                &options.target_agent_ids,
                &options.conflict_policy,
                None,
            )?;
            results.extend(fanout_res);
            installed_count += 1;
        }

        self.invalidate_installed_cache();

        // 触发多端同步上传
        self.sync_manager.trigger();

        Ok(crate::models::RemoteInstallResult {
            url: options.url,
            results,
            message: format!("成功安装 {} 个技能", installed_count),
        })
    }

    /// 从远程检测到的配置或手动配置安装 MCP Server
    pub fn install_remote_mcp(
        &self,
        options: crate::models::RemoteMcpInstallOptions,
    ) -> AppResult<Vec<crate::models::McpOperationResult>> {
        if options.target_agent_ids.is_empty() {
            return Err(AppError::Message("请至少选择一个目标 Agent。".into()));
        }
        let agents = self.list_agents()?;
        self.mcp().add_mcp_server(
            &agents,
            &options.target_agent_ids,
            &options.config,
            options.conflict_policy,
        )
    }

    /// 将 Skills Manager 自身注册为目标 Agent 的 MCP Server
    pub fn register_self_as_mcp(
        &self,
        target_agent_ids: &[String],
    ) -> AppResult<Vec<crate::models::McpOperationResult>> {
        let exe_path = std::env::current_exe()
            .map_err(|e| AppError::Message(format!("无法获取当前程序路径: {}", e)))?;
        let config = crate::models::McpServerConfig {
            name: "skills-manager".to_string(),
            transport: crate::models::McpTransport::Stdio,
            command: Some(exe_path.to_string_lossy().to_string()),
            args: vec!["mcp".to_string()],
            env: HashMap::new(),
            url: None,
            headers: HashMap::new(),
            disabled: false,
            timeout_sec: None,
        };
        let agents = self.list_agents()?;
        self.mcp().add_mcp_server(
            &agents,
            target_agent_ids,
            &config,
            crate::models::ConflictPolicy::BackupOverwrite,
        )
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
            supports_universal: false,
        };
        service.add_agent(profile).unwrap();
        add_test_hub(&service);
        service
    }

    /// 注册临时 Universal 中枢；TempDir 泄漏以保证测试期间路径有效。
    fn add_test_hub(service: &AppService) {
        let hub = tempfile::tempdir().unwrap();
        let path = hub.path().to_string_lossy().to_string();
        std::mem::forget(hub);
        service
            .add_agent(AgentProfile {
                id: format!("universal:{path}"),
                name: "Universal Hub".into(),
                agent_type: crate::models::AgentType::Universal,
                skills_path: path,
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            })
            .unwrap();
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
    fn migration_copies_old_hub_without_overwriting_new_hub() {
        let temp = tempfile::tempdir().unwrap();
        let old_hub = temp.path().join("old-hub");
        let new_hub = temp.path().join("program").join("skills");
        fs::create_dir_all(old_hub.join("legacy-skill")).unwrap();
        fs::create_dir_all(old_hub.join("existing-skill")).unwrap();
        fs::create_dir_all(new_hub.join("existing-skill")).unwrap();
        fs::write(old_hub.join("legacy-skill").join("SKILL.md"), "legacy").unwrap();
        fs::write(old_hub.join("existing-skill").join("SKILL.md"), "old").unwrap();
        fs::write(new_hub.join("existing-skill").join("SKILL.md"), "new").unwrap();
        fs::write(old_hub.join(".skill-lock.json"), "{}").unwrap();

        let allowed = HashSet::from(["legacy-skill".to_string(), "existing-skill".to_string()]);
        let migrated = migrate_hub_contents(&old_hub, &new_hub, &allowed).unwrap();

        assert_eq!(migrated, vec!["legacy-skill"]);
        assert_eq!(fs::read_to_string(new_hub.join("legacy-skill").join("SKILL.md")).unwrap(), "legacy");
        assert_eq!(fs::read_to_string(new_hub.join("existing-skill").join("SKILL.md")).unwrap(), "new");
        assert!(!new_hub.join(".skill-lock.json").exists());
        assert!(old_hub.join("legacy-skill").join("SKILL.md").exists());
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
            supports_universal: false,
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
            .import_uploaded_files("demo", &files, &["test-agent".into()], ConflictPolicy::Skip, true)
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
            .import_uploaded_files("demo", &files, &["test-agent".into()], ConflictPolicy::Skip, true)
            .unwrap();
        let result = service
            .import_uploaded_files("demo", &files, &["test-agent".into()], ConflictPolicy::Skip, true)
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
            supports_universal: false,
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
            supports_universal: false,
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
            true,
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
                true,
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
        add_test_hub(&service);
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
                supports_universal: false,
            },
            AgentProfile {
                id: "agent-b".into(),
                name: "Agent B".into(),
                agent_type: crate::models::AgentType::Custom,
                skills_path: agent_b_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            },
            AgentProfile {
                id: "target".into(),
                name: "Target".into(),
                agent_type: crate::models::AgentType::Custom,
                skills_path: target_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
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
                true,
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
        add_test_hub(&service);
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
                supports_universal: false,
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
                    supports_universal: false,
                })
                .unwrap();
        }

        let results = service
            .sync_grouped_skill(
                "Demo Skill",
                Some("source"),
                vec!["ok-agent".into(), "bad-agent".into()],
                ConflictPolicy::BackupOverwrite,
                true,
            )
            .unwrap();

        // 两个 agent 都返回结果：已有独立目录保持不变并跳过。
        assert_eq!(results.len(), 2);
        let ok = results.iter().find(|r| r.agent_id == "ok-agent").unwrap();
        assert_eq!(ok.action, "installed");
        let bad = results.iter().find(|r| r.agent_id == "bad-agent").unwrap();
        assert_eq!(bad.action, "skipped");
        assert_eq!(fs::read_to_string(bad_root.path().join("demo")).unwrap(), "occupied");
    }

    #[test]
    fn syncs_skill_to_claude_cowork_and_registers_manifest() {
        let service = AppService::in_memory().unwrap();
        add_test_hub(&service);
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
                supports_universal: false,
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
                true,
            )
            .unwrap();
        let second = service
            .sync_grouped_skill(
                "academic-paper",
                Some("source"),
                vec![cowork.id.clone()],
                ConflictPolicy::BackupOverwrite,
                true,
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

    #[test]
    fn sync_materializes_into_hub_then_links_to_target() {
        let service = AppService::in_memory().unwrap();
        let hub_root = tempfile::tempdir().unwrap();
        let source_root = tempfile::tempdir().unwrap();
        let target_root = tempfile::tempdir().unwrap();
        write_agent_skill(
            source_root.path(),
            "demo",
            Some("Demo Skill"),
            Some("1.0.0"),
            "# Demo Skill",
        );
        service
            .add_agent(AgentProfile {
                id: "hub".into(),
                name: "Universal Hub".into(),
                agent_type: AgentType::Universal,
                skills_path: hub_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "source".into(),
                name: "Source".into(),
                agent_type: AgentType::Custom,
                skills_path: source_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "target".into(),
                name: "Target".into(),
                agent_type: AgentType::Custom,
                skills_path: target_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();

        let results = service
            .sync_grouped_skill(
                "Demo Skill",
                Some("source"),
                vec!["target".into()],
                ConflictPolicy::BackupOverwrite,
                true,
            )
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "installed");
        assert!(hub_root.path().join("demo").join("SKILL.md").exists());
        let target_demo = target_root.path().join("demo");
        assert!(target_demo.exists());
        // 中枢实体 + 目标应可读到同一内容（软链或复制）
        assert_eq!(
            fs::read_to_string(target_demo.join("SKILL.md")).unwrap(),
            "# Demo Skill"
        );
    }

    #[test]
    fn hub_only_install_and_target_selection_do_not_touch_independent_skill() {
        let service = AppService::in_memory().unwrap();
        let hub = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        register_hub(&service, hub.path());
        for (id, path) in [("source", source.path()), ("target", target.path())] {
            service.add_agent(AgentProfile { id: id.into(), name: id.into(), agent_type: AgentType::Custom,
                skills_path: path.to_string_lossy().to_string(), adapter_config: None,
                user_tags: Vec::new(), supports_universal: false }).unwrap();
        }
        write_agent_skill(source.path(), "demo", Some("Demo Skill"), None, "# managed");
        let result = service.sync_grouped_skill("Demo Skill", Some("source"), vec![], ConflictPolicy::Skip, true).unwrap();
        assert!(result.is_empty());
        assert!(hub.path().join("demo/SKILL.md").exists());
        assert!(!target.path().join("demo").exists());

        write_agent_skill(target.path(), "own", Some("Demo Skill"), None, "# independent");
        let result = service.set_hub_skill_targets("Demo Skill", &["target".into()]).unwrap();
        assert_eq!(result[0].action, "skipped");
        assert!(!target.path().join("demo").exists());
        assert_eq!(fs::read_to_string(target.path().join("own/SKILL.md")).unwrap(), "# independent");
        service.set_hub_skill_targets("Demo Skill", &[]).unwrap();
        assert!(target.path().join("own").exists());
        service.uninstall_skill("Demo Skill", "target").unwrap();
        assert!(!target.path().join("own").exists());
    }

    #[test]
    fn unselect_removes_only_verified_manager_target_and_keeps_modified_copy() {
        let service = AppService::in_memory().unwrap();
        let hub = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        register_hub(&service, hub.path());
        service.add_agent(AgentProfile { id: "target".into(), name: "Target".into(), agent_type: AgentType::Custom,
            skills_path: target.path().to_string_lossy().to_string(), adapter_config: None,
            user_tags: Vec::new(), supports_universal: false }).unwrap();
        write_agent_skill(hub.path(), "demo", Some("Demo Skill"), None, "# hub");
        service.set_hub_skill_targets("Demo Skill", &["target".into()]).unwrap();
        assert!(target.path().join("demo").exists());
        service.set_hub_skill_targets("Demo Skill", &[]).unwrap();
        assert!(fs::symlink_metadata(target.path().join("demo")).is_err());
        assert!(hub.path().join("demo").exists());

        write_agent_skill(target.path(), "demo", Some("Demo Skill"), None, "# copy");
        service.store.upsert_managed_target(ManagedTargetRecord {
            skill_key: normalize_title("Demo Skill"), dir_name: "demo".into(), agent_id: "target".into(),
            target_path: target.path().join("demo").to_string_lossy().to_string(),
            method: "copy".into(), fingerprint: hash_dir(&target.path().join("demo")).unwrap(),
        }).unwrap();
        fs::write(target.path().join("demo/SKILL.md"), "# agent edited").unwrap();
        let result = service.set_hub_skill_targets("Demo Skill", &[]).unwrap();
        assert_eq!(result[0].action, "skipped");
        assert_eq!(fs::read_to_string(target.path().join("demo/SKILL.md")).unwrap(), "# agent edited");
        assert!(service.store.list_managed_targets().unwrap().is_empty());
    }

    #[test]
    fn background_update_keeps_modified_copy_and_records_notice() {
        let service = AppService::in_memory().unwrap();
        let hub = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        register_hub(&service, hub.path());
        service.add_agent(AgentProfile { id: "target".into(), name: "Target".into(), agent_type: AgentType::Custom,
            skills_path: target.path().to_string_lossy().to_string(), adapter_config: None,
            user_tags: Vec::new(), supports_universal: false }).unwrap();
        write_agent_skill(hub.path(), "demo", Some("Demo Skill"), None, "# hub");
        write_agent_skill(target.path(), "demo", Some("Demo Skill"), None, "# original copy");
        service.store.upsert_managed_target(ManagedTargetRecord {
            skill_key: normalize_title("Demo Skill"), dir_name: "demo".into(), agent_id: "target".into(),
            target_path: target.path().join("demo").to_string_lossy().to_string(),
            method: "copy".into(), fingerprint: hash_dir(&target.path().join("demo")).unwrap(),
        }).unwrap();
        fs::write(target.path().join("demo/SKILL.md"), "# edited by agent").unwrap();
        service.reconcile_from_hub(&["demo".into()]).unwrap();
        assert_eq!(fs::read_to_string(target.path().join("demo/SKILL.md")).unwrap(), "# edited by agent");
        assert!(service.list_pending_hub_skills().unwrap()[0].message.as_ref().unwrap().contains("独立安装"));
    }

    #[test]
    fn legacy_migration_accepts_only_unchanged_recorded_copies() {
        let service = AppService::in_memory().unwrap();
        let hub = tempfile::tempdir().unwrap();
        let agent_root = tempfile::tempdir().unwrap();
        service.add_agent(AgentProfile { id: "old".into(), name: "Old".into(), agent_type: AgentType::Custom,
            skills_path: agent_root.path().to_string_lossy().to_string(), adapter_config: None,
            user_tags: Vec::new(), supports_universal: false }).unwrap();
        write_agent_skill(agent_root.path(), "verified", Some("Verified"), None, "# original");
        write_agent_skill(agent_root.path(), "modified", Some("Modified"), None, "# original");
        write_agent_skill(agent_root.path(), "unknown", Some("Unknown"), None, "# own");
        let records = ["verified", "modified"].iter().map(|name| {
            let path = agent_root.path().join(name);
            InstallRecordInput { agent_id: "old".into(), skill_id: if *name == "verified" { "Verified" } else { "Modified" }.into(),
                fingerprint: hash_dir(&path).unwrap(), target_path: path.to_string_lossy().to_string(),
                action: "installed".into(), backup_path: None }
        }).collect::<Vec<_>>();
        service.store.record_installs(&records).unwrap();
        fs::write(agent_root.path().join("modified/SKILL.md"), "# edited").unwrap();
        service.migrate_legacy_install_records(hub.path(), &service.list_saved_agents().unwrap()).unwrap();
        assert!(hub.path().join("verified").exists());
        assert!(!hub.path().join("modified").exists());
        assert!(!hub.path().join("unknown").exists());
        assert_eq!(service.store.list_managed_targets().unwrap().len(), 1);
        assert!(agent_root.path().join("verified").exists());
    }

    #[test]
    fn all_agents_receive_hub_skill_link_on_fanout() {
        let service = AppService::in_memory().unwrap();
        let hub_root = tempfile::tempdir().unwrap();
        let source_root = tempfile::tempdir().unwrap();
        let native_root = tempfile::tempdir().unwrap();
        write_agent_skill(
            source_root.path(),
            "demo",
            Some("Demo Skill"),
            Some("1.0.0"),
            "# Demo Skill",
        );
        service
            .add_agent(AgentProfile {
                id: "hub".into(),
                name: "Universal Hub".into(),
                agent_type: AgentType::Universal,
                skills_path: hub_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "source".into(),
                name: "Source".into(),
                agent_type: AgentType::Custom,
                skills_path: source_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "native".into(),
                name: "Native".into(),
                agent_type: AgentType::Custom,
                skills_path: native_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            })
            .unwrap();

        let results = service
            .sync_grouped_skill(
                "Demo Skill",
                Some("source"),
                vec!["native".into()],
                ConflictPolicy::BackupOverwrite,
                true,
            )
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "installed");
        assert!(native_root.path().join("demo").join("SKILL.md").exists());
        assert!(hub_root.path().join("demo").join("SKILL.md").exists());
    }

    #[test]
    fn to_hub_false_is_rejected_without_changing_directories() {
        let service = AppService::in_memory().unwrap();
        let hub_root = tempfile::tempdir().unwrap();
        let source_root = tempfile::tempdir().unwrap();
        let target_root = tempfile::tempdir().unwrap();
        write_agent_skill(
            source_root.path(),
            "demo",
            Some("Demo Skill"),
            Some("1.0.0"),
            "# Demo Skill",
        );
        service
            .add_agent(AgentProfile {
                id: "hub".into(),
                name: "Universal Hub".into(),
                agent_type: AgentType::Universal,
                skills_path: hub_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "source".into(),
                name: "Source".into(),
                agent_type: AgentType::Custom,
                skills_path: source_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "target".into(),
                name: "Target".into(),
                agent_type: AgentType::Custom,
                skills_path: target_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();

        let result = service
            .sync_grouped_skill(
                "Demo Skill",
                Some("source"),
                vec!["target".into()],
                ConflictPolicy::BackupOverwrite,
                false,
            )
            ;

        assert!(result.is_err());
        let target_demo = target_root.path().join("demo");
        assert!(!target_demo.exists());
        assert!(!hub_root.path().join("demo").exists());
    }

    #[test]
    fn to_hub_false_import_is_rejected() {
        let service = AppService::in_memory().unwrap();
        let agent_dir = tempfile::tempdir().unwrap();
        service
            .add_agent(AgentProfile {
                id: "test-agent".into(),
                name: "Test Agent".into(),
                agent_type: AgentType::Custom,
                skills_path: agent_dir.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();
        let upload = tempfile::tempdir().unwrap();
        write_demo_skill(upload.path(), "demo");
        let files = collect_upload_files(upload.path());

        let result = service
            .import_uploaded_files(
                "demo",
                &files,
                &["test-agent".into()],
                ConflictPolicy::Skip,
                false,
            )
            ;

        assert!(result.is_err());
        assert!(!agent_dir.path().join("demo").exists());
    }

    fn register_hub(service: &AppService, hub: &Path) {
        service
            .add_agent(AgentProfile {
                id: format!("universal:{}", hub.to_string_lossy()),
                name: "Universal Hub".into(),
                agent_type: AgentType::Universal,
                skills_path: hub.to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            })
            .unwrap();
    }

    fn test_sync_config() -> SyncConfig {
        SyncConfig {
            enabled: true,
            endpoint: "local".into(),
            bucket: "bucket".into(),
            access_key_id: "key".into(),
            encrypt: true,
            ..SyncConfig::default()
        }
    }

    #[test]
    fn sync_round_trip_persists_hub_and_fans_out() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = crate::sync::LocalDirTransport::new(bucket_dir.path()).unwrap();

        // 设备 A：中枢里有 demo
        let hub_a = tempfile::tempdir().unwrap();
        let service_a = AppService::in_memory().unwrap();
        register_hub(&service_a, hub_a.path());
        write_agent_skill(
            hub_a.path(),
            "demo",
            Some("Demo Skill"),
            Some("1.0.0"),
            "# Demo Skill",
        );
        service_a
            .sync_set_config(test_sync_config(), Some("secret".into()), Some("pw".into()))
            .unwrap();
        let status_a = service_a
            .sync_run_with(&transport, &service_a.sync_get_config().unwrap())
            .unwrap();
        assert!(status_a.last_error.is_none());

        // 设备 B：空中枢 + 一个自定义 agent
        let hub_b = tempfile::tempdir().unwrap();
        let agent_b = tempfile::tempdir().unwrap();
        let service_b = AppService::in_memory().unwrap();
        register_hub(&service_b, hub_b.path());
        service_b
            .add_agent(AgentProfile {
                id: "target".into(),
                name: "Target".into(),
                agent_type: AgentType::Custom,
                skills_path: agent_b.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();
        service_b
            .sync_set_config(test_sync_config(), Some("secret".into()), Some("pw".into()))
            .unwrap();

        let status_b = service_b
            .sync_run_with(&transport, &service_b.sync_get_config().unwrap())
            .unwrap();
        assert!(status_b.last_error.is_none());
        assert!(hub_b.path().join("demo").join("SKILL.md").exists());
        // 新设备先入中枢，等待本机显式选择。
        assert!(!agent_b.path().join("demo").exists());
        assert_eq!(service_b.list_pending_hub_skills().unwrap().len(), 1);
        service_b.set_hub_skill_targets("Demo Skill", &["target".into()]).unwrap();
        assert!(agent_b.path().join("demo").exists());
    }

    #[test]
    fn sync_status_reports_configured_and_conflicts() {
        let service = AppService::in_memory().unwrap();
        assert!(!service.sync_status().unwrap().configured);
        service
            .sync_set_config(test_sync_config(), Some("secret".into()), Some("pw".into()))
            .unwrap();
        let status = service.sync_status().unwrap();
        assert!(status.configured);
        assert!(status.enabled);
        assert_eq!(status.pending_conflicts, 0);
    }

    #[test]
    fn sync_from_symlink_source_materializes_real_hub_copy() {
        let service = AppService::in_memory().unwrap();
        let hub_root = tempfile::tempdir().unwrap();
        let real_root = tempfile::tempdir().unwrap();
        let source_root = tempfile::tempdir().unwrap();
        let target_root = tempfile::tempdir().unwrap();
        write_agent_skill(
            real_root.path(),
            "demo",
            Some("Demo Skill"),
            Some("1.0.0"),
            "# Demo Skill",
        );
        // source 的 demo 是指向 real 的链接（模拟历史 Agent→Agent 软链）
        crate::hash::symlink_or_copy_dir(
            &real_root.path().join("demo"),
            &source_root.path().join("demo"),
        )
        .unwrap();
        service
            .add_agent(AgentProfile {
                id: "hub".into(),
                name: "Universal Hub".into(),
                agent_type: AgentType::Universal,
                skills_path: hub_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "source".into(),
                name: "Source".into(),
                agent_type: AgentType::Custom,
                skills_path: source_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "target".into(),
                name: "Target".into(),
                agent_type: AgentType::Custom,
                skills_path: target_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();

        service
            .sync_grouped_skill(
                "Demo Skill",
                Some("source"),
                vec!["target".into()],
                ConflictPolicy::BackupOverwrite,
                true,
            )
            .unwrap();

        assert_eq!(
            fs::read_to_string(hub_root.path().join("demo").join("SKILL.md")).unwrap(),
            "# Demo Skill"
        );
    }

    #[test]
    fn hub_skill_requires_agent_link_for_coverage() {
        let service = AppService::in_memory().unwrap();
        let hub_root = tempfile::tempdir().unwrap();
        let native_root = tempfile::tempdir().unwrap();
        let other_root = tempfile::tempdir().unwrap();
        write_agent_skill(
            hub_root.path(),
            "demo",
            Some("Demo Skill"),
            Some("1.0.0"),
            "# Demo Skill",
        );
        service
            .add_agent(AgentProfile {
                id: "hub".into(),
                name: "Universal Hub".into(),
                agent_type: AgentType::Universal,
                skills_path: hub_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "native".into(),
                name: "Native".into(),
                agent_type: AgentType::Custom,
                skills_path: native_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: true,
            })
            .unwrap();
        service
            .add_agent(AgentProfile {
                id: "other".into(),
                name: "Other".into(),
                agent_type: AgentType::Custom,
                skills_path: other_root.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();

        let skills = service.scan_agent_skills().unwrap();
        let demo = skills
            .into_iter()
            .find(|skill| skill.title == "Demo Skill")
            .unwrap();
        assert!(demo.is_universal);
        assert!(demo.missing_agent_ids.contains(&"native".to_string()));
        assert!(demo.missing_agent_ids.contains(&"other".to_string()));
    }

    #[test]
    fn legacy_hub_path_does_not_imply_program_hub_coverage() {
        let service = AppService::in_memory().unwrap();
        let hub_root = tempfile::tempdir().unwrap();
        let path = hub_root.path().to_string_lossy().to_string();
        let saved = service
            .add_agent(AgentProfile {
                id: format!("custom:{path}"),
                name: "Custom Hub Path".into(),
                agent_type: AgentType::Custom,
                skills_path: path,
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();
        // 旧版默认中枢路径不再让 Agent 被视作已覆盖。
        let agents = service.list_agents().unwrap();
        let _ = agents;
        let _ = saved;

        let fake_home_style = hub_root.path().join(".agents").join("skills");
        fs::create_dir_all(&fake_home_style).unwrap();
        let path2 = fake_home_style.to_string_lossy().to_string();
        let profile = service
            .add_agent(AgentProfile {
                id: format!("custom2:{path2}"),
                name: "Agents Path".into(),
                agent_type: AgentType::Custom,
                skills_path: path2,
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();
        assert!(!profile.supports_universal);
        let listed = service
            .list_agents()
            .unwrap()
            .into_iter()
            .find(|a| a.id.starts_with("custom2:"))
            .unwrap();
        assert!(!listed.supports_universal);
    }

    #[test]
    fn add_agent_fills_default_name_for_builtin_type() {
        let service = AppService::in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let saved = service
            .add_agent(AgentProfile {
                id: String::new(),
                name: "   ".into(),
                agent_type: AgentType::Codex,
                skills_path: dir.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();
        assert_eq!(saved.name, "Codex");
    }

    #[test]
    fn add_agent_still_requires_path_for_custom_type() {
        let service = AppService::in_memory().unwrap();
        let result = service.add_agent(AgentProfile {
            id: String::new(),
            name: "My Agent".into(),
            agent_type: AgentType::Custom,
            skills_path: "  ".into(),
            adapter_config: None,
            user_tags: Vec::new(),
            supports_universal: false,
        });
        assert!(result.is_err());
    }

    fn local_sync_config(bucket: &Path) -> SyncConfig {
        SyncConfig {
            enabled: true,
            endpoint: format!("local://{}", bucket.display()),
            bucket: String::new(),
            access_key_id: String::new(),
            encrypt: true,
            poll_secs: 3600,
            ..SyncConfig::default()
        }
    }

    fn make_device(
        data_dir: &Path,
        hub: &Path,
        secrets: Arc<crate::sync::MemorySecretStore>,
        bucket: &Path,
    ) -> AppService {
        secrets
            .set(crate::sync::ENCRYPT_PASSWORD, "same-password")
            .unwrap();
        let service =
            AppService::from_store(Arc::new(AppStore::with_data_dir(data_dir.to_path_buf()).unwrap()), secrets)
                .unwrap();
        register_hub(&service, hub);
        service
            .sync_set_config(local_sync_config(bucket), None, Some("same-password".into()))
            .unwrap();
        service
    }

    #[test]
    fn local_endpoint_builds_dir_transport_and_skips_s3_fields() {
        let dir = tempfile::tempdir().unwrap();
        let config = SyncConfig {
            enabled: true,
            endpoint: format!("local://{}", dir.path().display()),
            bucket: "nested".into(),
            encrypt: false,
            ..SyncConfig::default()
        };
        let secrets = crate::sync::MemorySecretStore::new();
        let transport = build_transport_from(&config, &secrets).unwrap();
        transport.test_connection().unwrap();
        assert!(dir.path().join("nested").exists());

        let service = AppService::in_memory().unwrap();
        service
            .sync_set_config(
                SyncConfig {
                    encrypt: false,
                    ..config.clone()
                },
                None,
                None,
            )
            .unwrap();
        // 本地目录不要求 Access Key / Secret
        assert!(service.sync_status().unwrap().configured);
        let message = service.sync_test_connection(config).unwrap();
        assert!(message.contains("本地目录可用"));
    }

    #[test]
    fn sync_now_local_dir_round_trip_between_two_devices() {
        let bucket = tempfile::tempdir().unwrap();
        let data_a = tempfile::tempdir().unwrap();
        let data_b = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let hub_b = tempfile::tempdir().unwrap();
        let agent_b = tempfile::tempdir().unwrap();

        let secrets_a = Arc::new(crate::sync::MemorySecretStore::new());
        let secrets_b = Arc::new(crate::sync::MemorySecretStore::new());
        let service_a = make_device(data_a.path(), hub_a.path(), secrets_a, bucket.path());
        let service_b = make_device(data_b.path(), hub_b.path(), secrets_b, bucket.path());

        service_b
            .add_agent(AgentProfile {
                id: "target-b".into(),
                name: "Target B".into(),
                agent_type: AgentType::Custom,
                skills_path: agent_b.path().to_string_lossy().to_string(),
                adapter_config: None,
                user_tags: Vec::new(),
                supports_universal: false,
            })
            .unwrap();

        write_agent_skill(
            hub_a.path(),
            "demo-local",
            Some("Demo Local"),
            Some("1.0.0"),
            "# Demo Local\n\nfrom device A",
        );

        let status_a = service_a.sync_now().unwrap();
        assert!(status_a.last_error.is_none(), "{:?}", status_a.last_error);
        let status_b = service_b.sync_now().unwrap();
        assert!(status_b.last_error.is_none(), "{:?}", status_b.last_error);

        assert!(hub_b.path().join("demo-local").join("SKILL.md").exists());
        assert!(!agent_b.path().join("demo-local").exists());
        assert_eq!(service_b.list_pending_hub_skills().unwrap().len(), 1);
        service_b.set_hub_skill_targets("Demo Local", &["target-b".into()]).unwrap();
        assert!(agent_b.path().join("demo-local").exists());
        // 设备身份应不同
        assert_ne!(
            service_a.sync_status().unwrap().device_id,
            service_b.sync_status().unwrap().device_id
        );
    }

    #[test]
    fn sync_now_local_dir_reports_conflict_when_both_edit() {
        let bucket = tempfile::tempdir().unwrap();
        let data_a = tempfile::tempdir().unwrap();
        let data_b = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let hub_b = tempfile::tempdir().unwrap();

        let service_a = make_device(
            data_a.path(),
            hub_a.path(),
            Arc::new(crate::sync::MemorySecretStore::new()),
            bucket.path(),
        );
        let service_b = make_device(
            data_b.path(),
            hub_b.path(),
            Arc::new(crate::sync::MemorySecretStore::new()),
            bucket.path(),
        );

        write_agent_skill(
            hub_a.path(),
            "demo-conflict",
            Some("Demo Conflict"),
            Some("1.0.0"),
            "# v1",
        );
        service_a.sync_now().unwrap();
        service_b.sync_now().unwrap();
        assert_eq!(
            fs::read_to_string(hub_b.path().join("demo-conflict").join("SKILL.md")).unwrap(),
            "# v1"
        );

        // A/B 各自改内容后同步 → 双端修改冲突
        write_agent_skill(
            hub_a.path(),
            "demo-conflict",
            Some("Demo Conflict"),
            Some("1.0.0"),
            "# v2-from-A",
        );
        service_a.sync_now().unwrap();
        write_agent_skill(
            hub_b.path(),
            "demo-conflict",
            Some("Demo Conflict"),
            Some("1.0.0"),
            "# v2-from-B",
        );
        let status_b = service_b.sync_now().unwrap();
        let conflicts = service_b.sync_list_conflicts().unwrap();
        assert!(
            !conflicts.is_empty() || status_b.pending_conflicts > 0,
            "expected conflict after both sides edit, status={status_b:?} conflicts={conflicts:?}"
        );

        if !conflicts.is_empty() {
            let skill_id = conflicts[0].skill_id.clone();
            service_b
                .sync_resolve_conflict(&skill_id, SyncConflictChoice::Remote)
                .unwrap();
            let after = service_b.sync_list_conflicts().unwrap();
            assert!(after.iter().all(|c| c.skill_id != skill_id));
        }
    }

    #[test]
    fn inspect_and_install_remote_source_local_dir() {
        let temp = tempfile::tempdir().unwrap();
        let agent_dir = temp.path().join("agent-skills");
        fs::create_dir_all(&agent_dir).unwrap();
        let service = test_service_with_agent(&agent_dir);

        let source_repo = temp.path().join("mock-repo");
        let skill_dir = source_repo.join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Remote Skill\ndescription: A remote skill\nversion: 1.2.0\n---\n# Docs",
        )
        .unwrap();

        let inspection = service
            .inspect_remote_source(&source_repo.to_string_lossy())
            .unwrap();
        assert_eq!(inspection.skills.len(), 1);
        assert_eq!(inspection.skills[0].title, "Remote Skill");
        assert!(!inspection.available_agents.is_empty());

        let res = service
            .install_remote_source(crate::models::RemoteInstallOptions {
                url: source_repo.to_string_lossy().to_string(),
                target_agent_ids: vec!["test-agent".to_string()],
                conflict_policy: ConflictPolicy::BackupOverwrite,
                to_hub: Some(true),
                selected_skills: vec![],
            })
            .unwrap();

        assert!(!res.results.is_empty());
        assert!(agent_dir.join("my-skill").exists());
    }
}
