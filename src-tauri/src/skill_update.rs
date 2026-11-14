//! 从记录的 Git 仓库或本地目录检查并更新中枢 Skill。
//! 新内容只写中枢，再按本机可核实的管理器目标刷新分发，并触发多端同步上传。

use crate::{
    error::{AppError, AppResult},
    hash::{copy_dir_all, copy_skill_content, hash_skill_content, remove_dir_or_symlink},
    models::{SkillUpdateInfo, SkillUpdateResult, SkillUpdateState},
    remote_source::{inspect_directory, parse_git_url, RemoteFetcher},
    service::AppService,
    skill_scan::{read_agent_skill_info, write_skill_lock_entry},
    store::SkillSourceRecord,
    util::{normalize_title, safe_label},
};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

struct TrackedSkill {
    dir_name: String,
    hub_path: PathBuf,
    title: String,
    record: SkillSourceRecord,
}

impl AppService {
    /// 安装后记录来源；仅当中枢内容与上游一致时才写入基准指纹，避免把跳过/冲突的旧内容当成上游版本。
    pub(crate) fn record_skill_source(
        &self,
        hub_dir_name: &str,
        hub_skill_path: &Path,
        upstream_dir: &Path,
        source_url: &str,
        relative_path: &str,
    ) -> AppResult<()> {
        let upstream_hash = hash_skill_content(upstream_dir)?;
        let hub_hash = hash_skill_content(hub_skill_path)?;
        self.store.set_skill_source(
            hub_dir_name,
            SkillSourceRecord {
                source_url: source_url.to_string(),
                relative_path: Some(relative_path.to_string()),
                upstream_hash: (upstream_hash == hub_hash).then_some(upstream_hash),
                updated_at: Some(chrono::Utc::now().to_rfc3339()),
            },
        )
    }

    /// 检查所有带来源记录的中枢 Skill；同一仓库只拉取一次，单个仓库失败不影响其他仓库。
    pub fn check_skill_updates(&self) -> AppResult<Vec<SkillUpdateInfo>> {
        let tracked = self.tracked_hub_skills()?;
        let mut infos = Vec::new();
        for (url, group) in group_by_source(tracked) {
            let base = match self.fetch_update_source(&url) {
                Ok(base) => base,
                Err(error) => {
                    for skill in group {
                        infos.push(error_info(&skill, format!("拉取来源失败：{error}")));
                    }
                    continue;
                }
            };
            for skill in group {
                infos.push(match self.inspect_skill_update(&skill, &base) {
                    Ok(info) => info,
                    Err(error) => error_info(&skill, error.to_string()),
                });
            }
        }
        infos.sort_by(|a, b| normalize_title(&a.title).cmp(&normalize_title(&b.title)));
        Ok(infos)
    }

    /// 按中枢目录名更新；逐个隔离失败，结束后统一触发同步上传。
    pub fn update_skills_from_source(&self, dir_names: &[String]) -> AppResult<Vec<SkillUpdateResult>> {
        let wanted: HashSet<&str> = dir_names.iter().map(String::as_str).collect();
        let tracked: Vec<TrackedSkill> = self
            .tracked_hub_skills()?
            .into_iter()
            .filter(|skill| wanted.contains(skill.dir_name.as_str()))
            .collect();
        let mut results = Vec::new();
        for name in dir_names {
            if !tracked.iter().any(|skill| &skill.dir_name == name) {
                results.push(SkillUpdateResult {
                    title: name.clone(),
                    dir_name: name.clone(),
                    updated: false,
                    error: true,
                    backup_path: None,
                    message: "中枢中没有该 Skill 或没有记录来源，无法更新。".into(),
                });
            }
        }
        let mut any_updated = false;
        for (url, group) in group_by_source(tracked) {
            let base = match self.fetch_update_source(&url) {
                Ok(base) => base,
                Err(error) => {
                    for skill in group {
                        results.push(error_result(&skill, format!("拉取来源失败：{error}")));
                    }
                    continue;
                }
            };
            for skill in group {
                let result = self
                    .apply_skill_update(&skill, &base)
                    .unwrap_or_else(|error| error_result(&skill, error.to_string()));
                any_updated |= result.updated;
                results.push(result);
            }
        }
        if any_updated {
            self.invalidate_installed_cache();
            self.sync_manager.trigger();
        }
        Ok(results)
    }

    fn tracked_hub_skills(&self) -> AppResult<Vec<TrackedSkill>> {
        let hub = self.ensure_hub_agent()?;
        let hub_root = Path::new(&hub.skills_path);
        if !hub_root.is_dir() {
            return Ok(Vec::new());
        }
        let lock_sources = legacy_lock_git_sources();
        let mut tracked = Vec::new();
        for entry in fs::read_dir(hub_root)? {
            let entry = entry?;
            let path = entry.path();
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if dir_name.starts_with('.') || !path.is_dir() {
                continue;
            }
            let record = match self.store.skill_source(&dir_name)? {
                Some(record) => record,
                None => match lock_sources.get(&dir_name) {
                    Some(url) => SkillSourceRecord { source_url: url.clone(), ..Default::default() },
                    None => continue,
                },
            };
            if record.source_url.trim().is_empty() {
                continue;
            }
            let (title, _, _, _) = read_agent_skill_info(&path, false);
            let title = if title.trim().is_empty() { dir_name.clone() } else { title };
            tracked.push(TrackedSkill { dir_name, hub_path: path, title, record });
        }
        Ok(tracked)
    }

    /// 返回来源中 Skill 搜索的根目录（已应用 URL 中的子目录）。
    fn fetch_update_source(&self, url: &str) -> AppResult<PathBuf> {
        let trimmed = url.trim();
        let local = Path::new(trimmed);
        if local.is_dir() {
            return Ok(local.to_path_buf());
        }
        let parsed = parse_git_url(trimmed)?;
        let fetched = RemoteFetcher::new(self.remote_cache_root().join("updates")).fetch(&parsed)?;
        Ok(match parsed.subpath.as_deref() {
            Some(sub) => {
                let joined = fetched.join(crate::util::safe_relative_path(sub)?);
                if joined.exists() { joined } else { fetched }
            }
            None => fetched,
        })
    }

    fn inspect_skill_update(&self, skill: &TrackedSkill, base: &Path) -> AppResult<SkillUpdateInfo> {
        let (upstream_dir, relative_path) = locate_upstream_skill(base, skill)?;
        let upstream_hash = hash_skill_content(&upstream_dir)?;
        let hub_hash = hash_skill_content(&skill.hub_path)?;
        let baseline = skill.record.upstream_hash.as_deref();
        let (state, local_modified, message) = if upstream_hash == hub_hash {
            if baseline != Some(upstream_hash.as_str()) || skill.record.relative_path.is_none() {
                self.store.set_skill_source(
                    &skill.dir_name,
                    SkillSourceRecord {
                        relative_path: Some(relative_path),
                        upstream_hash: Some(upstream_hash.clone()),
                        ..skill.record.clone()
                    },
                )?;
            }
            (SkillUpdateState::UpToDate, false, None)
        } else if baseline == Some(upstream_hash.as_str()) {
            (
                SkillUpdateState::LocalModified,
                true,
                Some("上游没有新版本，中枢中的内容已在本机修改。".to_string()),
            )
        } else {
            let local_modified = baseline.is_some_and(|value| value != hub_hash);
            let message = if local_modified {
                Some("上游有新版本，且中枢内容已在本机修改；更新前会自动备份。".to_string())
            } else if baseline.is_none() {
                Some("中枢内容与上游不同（缺少安装基准，无法判断是否为本机修改）；更新前会自动备份。".to_string())
            } else {
                None
            };
            (SkillUpdateState::UpdateAvailable, local_modified, message)
        };
        Ok(SkillUpdateInfo {
            title: skill.title.clone(),
            dir_name: skill.dir_name.clone(),
            source_url: skill.record.source_url.clone(),
            state,
            local_modified,
            message,
        })
    }

    fn apply_skill_update(&self, skill: &TrackedSkill, base: &Path) -> AppResult<SkillUpdateResult> {
        let (upstream_dir, relative_path) = locate_upstream_skill(base, skill)?;
        let upstream_hash = hash_skill_content(&upstream_dir)?;
        let hub_hash = hash_skill_content(&skill.hub_path)?;
        let new_record = SkillSourceRecord {
            source_url: skill.record.source_url.clone(),
            relative_path: Some(relative_path),
            upstream_hash: Some(upstream_hash.clone()),
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
        };
        if upstream_hash == hub_hash {
            self.store.set_skill_source(&skill.dir_name, new_record)?;
            return Ok(SkillUpdateResult {
                title: skill.title.clone(),
                dir_name: skill.dir_name.clone(),
                updated: false,
                error: false,
                backup_path: None,
                message: "已是最新版本。".into(),
            });
        }

        let hub = self.ensure_hub_agent()?;
        let hub_root = Path::new(&hub.skills_path);
        // 点号开头的暂存目录不会被同步扫描，避免把半成品上传到其他设备
        let staging = hub_root.join(format!(".update-{}", uuid::Uuid::new_v4().simple()));
        if let Err(error) = copy_skill_content(&upstream_dir, &staging) {
            let _ = remove_dir_or_symlink(&staging);
            return Err(error);
        }
        let backup = self
            .backup_root()
            .join(safe_label(&hub.id))
            .join(safe_label(&skill.title))
            .join(chrono::Utc::now().format("%Y%m%d%H%M%S%3f").to_string());
        if let Err(error) = copy_dir_all(&skill.hub_path, &backup) {
            let _ = remove_dir_or_symlink(&staging);
            return Err(error);
        }
        if let Err(error) = remove_dir_or_symlink(&skill.hub_path) {
            let _ = remove_dir_or_symlink(&staging);
            return Err(AppError::Message(format!(
                "无法替换中枢目录（可能被占用）：{error}。原内容未改动，备份位于 {}",
                backup.display()
            )));
        }
        if let Err(error) = fs::rename(&staging, &skill.hub_path) {
            let restored = copy_dir_all(&backup, &skill.hub_path).is_ok();
            let _ = remove_dir_or_symlink(&staging);
            return Err(AppError::Message(format!(
                "写入新版本失败：{error}。{}",
                if restored { "已从备份恢复原内容。" } else { "请从备份手动恢复。" }
            )));
        }

        let _ = write_skill_lock_entry(
            &skill.dir_name,
            Some("git"),
            Some("git"),
            Some(&skill.record.source_url),
            &skill.hub_path,
        );
        self.store.set_skill_source(&skill.dir_name, new_record)?;
        self.reconcile_from_hub(&[skill.dir_name.clone()])?;

        let (new_title, _, _, _) = read_agent_skill_info(&skill.hub_path, false);
        let title = if new_title.trim().is_empty() { skill.title.clone() } else { new_title };
        Ok(SkillUpdateResult {
            title,
            dir_name: skill.dir_name.clone(),
            updated: true,
            error: false,
            backup_path: Some(backup.to_string_lossy().to_string()),
            message: "已更新到上游最新内容，原内容已备份。".into(),
        })
    }
}

fn group_by_source(tracked: Vec<TrackedSkill>) -> BTreeMap<String, Vec<TrackedSkill>> {
    let mut groups: BTreeMap<String, Vec<TrackedSkill>> = BTreeMap::new();
    for skill in tracked {
        groups.entry(skill.record.source_url.trim().to_string()).or_default().push(skill);
    }
    groups
}

/// 依次按记录的相对路径、目录名、标题匹配上游 Skill；仓库只有一个 Skill 时直接使用。
fn locate_upstream_skill(base: &Path, skill: &TrackedSkill) -> AppResult<(PathBuf, String)> {
    let (detected, _, _) = inspect_directory(base, None)?;
    let dir_of = |relative: &str| if relative.is_empty() { base.to_path_buf() } else { base.join(relative) };
    let found = skill
        .record
        .relative_path
        .as_deref()
        .and_then(|rel| detected.iter().find(|info| info.relative_path == rel))
        .or_else(|| {
            detected.iter().find(|info| {
                dir_of(&info.relative_path)
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy() == skill.dir_name)
            })
        })
        .or_else(|| {
            let key = normalize_title(&skill.title);
            detected.iter().find(|info| normalize_title(&info.title) == key)
        })
        .or_else(|| (detected.len() == 1).then(|| &detected[0]));
    let info = found.ok_or_else(|| {
        AppError::Message(format!("来源中找不到 Skill「{}」，可能已被移动或删除。", skill.title))
    })?;
    Ok((dir_of(&info.relative_path), info.relative_path.clone()))
}

fn error_info(skill: &TrackedSkill, message: String) -> SkillUpdateInfo {
    SkillUpdateInfo {
        title: skill.title.clone(),
        dir_name: skill.dir_name.clone(),
        source_url: skill.record.source_url.clone(),
        state: SkillUpdateState::Error,
        local_modified: false,
        message: Some(message),
    }
}

fn error_result(skill: &TrackedSkill, message: String) -> SkillUpdateResult {
    SkillUpdateResult {
        title: skill.title.clone(),
        dir_name: skill.dir_name.clone(),
        updated: false,
        error: true,
        backup_path: None,
        message,
    }
}

/// 本功能上线前通过 Git URL 安装的 Skill 只有 `.skill-lock.json` 记录，作为来源兜底。
/// 测试环境不读真实 home，保持隔离。
fn legacy_lock_git_sources() -> std::collections::HashMap<String, String> {
    #[cfg(test)]
    {
        std::collections::HashMap::new()
    }
    #[cfg(not(test))]
    {
        crate::skill_scan::load_skill_lock_map()
            .into_iter()
            .filter(|(_, entry)| entry.source_type.as_deref() == Some("git"))
            .filter_map(|(dir, entry)| entry.source_url.filter(|url| !url.trim().is_empty()).map(|url| (dir, url)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        models::{AgentProfile, AgentType, ConflictPolicy, RemoteInstallOptions, SkillUpdateState},
        service::AppService,
    };
    use std::{fs, path::Path};

    fn write_skill(dir: &Path, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("SKILL.md"), format!("---\nname: Remote Skill\ndescription: d\n---\n{body}")).unwrap();
    }

    fn setup() -> (AppService, tempfile::TempDir, std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let hub = temp.path().join("hub");
        let agent = temp.path().join("agent");
        let repo = temp.path().join("repo");
        fs::create_dir_all(&hub).unwrap();
        fs::create_dir_all(&agent).unwrap();
        let service = AppService::in_memory().unwrap();
        service.add_agent(AgentProfile {
            id: "hub".into(), name: "Hub".into(), agent_type: AgentType::Universal,
            skills_path: hub.to_string_lossy().to_string(), adapter_config: None,
            user_tags: Vec::new(), supports_universal: true,
        }).unwrap();
        service.add_agent(AgentProfile {
            id: "agent".into(), name: "Agent".into(), agent_type: AgentType::Custom,
            skills_path: agent.to_string_lossy().to_string(), adapter_config: None,
            user_tags: Vec::new(), supports_universal: false,
        }).unwrap();
        write_skill(&repo.join("my-skill"), "# v1");
        service.install_remote_source(RemoteInstallOptions {
            url: repo.to_string_lossy().to_string(),
            target_agent_ids: vec!["agent".into()],
            conflict_policy: ConflictPolicy::BackupOverwrite,
            to_hub: Some(true),
            selected_skills: vec![],
        }).unwrap();
        (service, temp, hub, agent, repo)
    }

    #[test]
    fn check_reports_up_to_date_then_update_available() {
        let (service, _temp, _hub, _agent, repo) = setup();
        let infos = service.check_skill_updates().unwrap();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].state, SkillUpdateState::UpToDate);

        write_skill(&repo.join("my-skill"), "# v2");
        let infos = service.check_skill_updates().unwrap();
        assert_eq!(infos[0].state, SkillUpdateState::UpdateAvailable);
        assert!(!infos[0].local_modified);
    }

    #[test]
    fn update_replaces_hub_content_and_refreshes_agent_target() {
        let (service, _temp, hub, agent, repo) = setup();
        write_skill(&repo.join("my-skill"), "# v2");
        let results = service.update_skills_from_source(&["my-skill".into()]).unwrap();
        assert!(results[0].updated, "{:?}", results[0]);
        let backup = results[0].backup_path.as_ref().unwrap();
        assert!(fs::read_to_string(Path::new(backup).join("SKILL.md")).unwrap().contains("# v1"));
        assert!(fs::read_to_string(hub.join("my-skill/SKILL.md")).unwrap().contains("# v2"));
        assert!(fs::read_to_string(agent.join("my-skill/SKILL.md")).unwrap().contains("# v2"));
        assert!(!hub.read_dir().unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".update-")));
        assert_eq!(service.check_skill_updates().unwrap()[0].state, SkillUpdateState::UpToDate);
    }

    #[test]
    fn local_edit_without_upstream_change_is_not_an_update() {
        let (service, _temp, hub, _agent, repo) = setup();
        fs::write(hub.join("my-skill/notes.md"), "mine").unwrap();
        let info = &service.check_skill_updates().unwrap()[0];
        assert_eq!(info.state, SkillUpdateState::LocalModified);

        write_skill(&repo.join("my-skill"), "# v2");
        let info = &service.check_skill_updates().unwrap()[0];
        assert_eq!(info.state, SkillUpdateState::UpdateAvailable);
        assert!(info.local_modified);
        let result = &service.update_skills_from_source(&["my-skill".into()]).unwrap()[0];
        assert!(result.updated);
        assert!(Path::new(result.backup_path.as_ref().unwrap()).join("notes.md").exists());
    }

    #[test]
    fn missing_source_is_reported_per_skill() {
        let (service, _temp, _hub, _agent, repo) = setup();
        fs::remove_dir_all(&repo).unwrap();
        let infos = service.check_skill_updates().unwrap();
        assert_eq!(infos[0].state, SkillUpdateState::Error);
        let results = service.update_skills_from_source(&["my-skill".into(), "unknown".into()]).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.error && !r.updated));
    }
}
