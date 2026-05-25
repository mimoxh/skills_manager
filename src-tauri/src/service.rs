use crate::{
    adapter::{adapter_for, built_in_adapters, AgentAdapter},
    error::{AppError, AppResult},
    hash::copy_dir_all,
    manifest::{read_skill, scan_repository},
    models::{
        AgentProfile, ConflictPolicy, ImportSkillFile, ImportSkillResult, InstallResult,
        InstallState, SkillSummary, SyncCandidate,
    },
    store::AppStore,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};
use zip::ZipArchive;

pub struct AppService {
    store: AppStore,
}

impl AppService {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            store: AppStore::new()?,
        })
    }

    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        Ok(Self {
            store: AppStore::in_memory()?,
        })
    }

    pub fn store(&self) -> &AppStore {
        &self.store
    }

    pub fn data_dir(&self) -> PathBuf {
        self.store.data_dir()
    }

    pub fn backup_root(&self) -> PathBuf {
        self.store.backup_root()
    }

    pub fn import_root(&self) -> PathBuf {
        self.store.import_root()
    }

    pub fn set_repository(&self, path: &str) -> AppResult<String> {
        let path = path.trim();
        if path.is_empty() {
            return Err(AppError::Message("主仓库路径不能为空".to_string()));
        }
        fs::create_dir_all(path)?;
        self.store.set_repository(path)?;
        Ok(path.to_string())
    }

    pub fn get_repository(&self) -> AppResult<String> {
        self.store.get_or_create_repository()
    }

    pub fn scan_skills(&self) -> AppResult<Vec<SkillSummary>> {
        scan_repository(Path::new(&self.get_repository()?))
    }

    pub fn detect_agents(&self) -> AppResult<Vec<AgentProfile>> {
        let mut agents = Vec::new();
        for adapter in built_in_adapters() {
            agents.extend(adapter.detect());
        }
        Ok(agents)
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

    pub fn list_install_state(&self) -> AppResult<Vec<InstallState>> {
        let skills = self.scan_skills().unwrap_or_default();
        let agents = self.list_agents()?;
        let mut states = Vec::new();
        for agent in agents {
            let adapter = adapter_for(&agent);
            for skill in &skills {
                let installed = self
                    .store
                    .installed_fingerprint(&agent.id, &skill.manifest.id)?;
                states.push(adapter.diff(skill, &agent, installed)?);
            }
        }
        Ok(states)
    }

    pub fn preview_sync(&self, agent_id: &str) -> AppResult<Vec<SyncCandidate>> {
        let skills = self.scan_skills()?;
        let agent = self
            .list_agents()?
            .into_iter()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;
        let adapter = adapter_for(&agent);
        let mut candidates = Vec::new();
        for skill in skills {
            if adapter.check_compatibility(&skill, &agent) {
                let installed = self
                    .store
                    .installed_fingerprint(&agent.id, &skill.manifest.id)?;
                let state = adapter.diff(&skill, &agent, installed)?;
                candidates.push(SyncCandidate {
                    skill,
                    states: vec![state],
                });
            }
        }
        Ok(candidates)
    }

    pub fn install_skills(
        &self,
        skill_ids: Vec<String>,
        agent_ids: Vec<String>,
        conflict_policy: ConflictPolicy,
    ) -> AppResult<Vec<InstallResult>> {
        let skills = self.scan_skills()?;
        let agents = self.list_agents()?;
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
                .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;
            let adapter = adapter_for(agent);
            for skill_id in &skill_ids {
                let skill = skill_map
                    .get(skill_id)
                    .ok_or_else(|| AppError::Message(format!("找不到 Skill: {}", skill_id)))?;
                let result =
                    adapter.install(skill, agent, conflict_policy.clone(), &self.backup_root())?;
                if result.action != "skipped" {
                    self.store.record_install(
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

    pub fn uninstall_skill(&self, skill_id: &str, agent_id: &str) -> AppResult<()> {
        let agent = self
            .list_agents()?
            .into_iter()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| AppError::Message(format!("找不到 Agent: {}", agent_id)))?;
        let adapter = adapter_for(&agent);
        let backup = adapter.uninstall(skill_id, &agent, &self.backup_root())?;
        let target_path = Path::new(&agent.skills_path)
            .join(skill_id)
            .to_string_lossy()
            .to_string();
        self.store.record_uninstall(
            agent_id,
            skill_id,
            &target_path,
            backup
                .as_ref()
                .map(|path| path.to_string_lossy())
                .as_deref(),
        )
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

    pub fn import_folder(&self, source_root: &Path) -> AppResult<ImportSkillResult> {
        let repository = PathBuf::from(self.get_repository()?);
        self.import_skill_dirs(source_root, &repository)
    }

    pub fn import_zip_file(&self, archive_path: &Path) -> AppResult<ImportSkillResult> {
        let bytes = fs::read(archive_path)?;
        let label = archive_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("zip");
        let source_root = self.unpack_zip_bytes(&bytes, label)?;
        self.import_folder(&source_root)
    }

    pub fn import_uploaded_files(
        &self,
        file_name: &str,
        files: &[ImportSkillFile],
    ) -> AppResult<ImportSkillResult> {
        if files.is_empty() {
            return Err(AppError::Message("上传内容为空".to_string()));
        }

        let source_root = if files.len() == 1 && file_name.to_ascii_lowercase().ends_with(".zip") {
            self.unpack_zip_bytes(&files[0].bytes, file_name)?
        } else {
            self.write_uploaded_files(files)?
        };
        self.import_folder(&source_root)
    }

    fn import_skill_dirs(
        &self,
        source_root: &Path,
        repository: &Path,
    ) -> AppResult<ImportSkillResult> {
        let dirs = self.manifest_source_dirs(source_root)?;
        if dirs.is_empty() {
            return Ok(ImportSkillResult {
                imported: 0,
                skipped: 0,
                message: "没有发现可识别的 skill manifest。".to_string(),
            });
        }

        let mut imported = 0;
        let mut skipped = 0;
        for source in dirs {
            let skill = read_skill(&self.manifest_path_for(&source)?)?;
            let target = repository.join(safe_relative_path(&skill.manifest.id)?);
            if target.exists() {
                skipped += 1;
                continue;
            }
            copy_dir_all(&source, &target)?;
            imported += 1;
        }

        Ok(ImportSkillResult {
            imported,
            skipped,
            message: format!("已导入 {} 个 skills，跳过 {} 个已存在 skills。", imported, skipped),
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

    fn unpack_zip_bytes(&self, bytes: &[u8], label: &str) -> AppResult<PathBuf> {
        let workspace = self.import_workspace(label)?;
        let extracted = workspace.join("expanded");
        fs::create_dir_all(&extracted)?;
        let mut archive = ZipArchive::new(Cursor::new(bytes))?;

        for index in 0..archive.len() {
            let mut file = archive.by_index(index)?;
            if file.is_dir() {
                continue;
            }
            let Some(enclosed) = file.enclosed_name().map(PathBuf::from) else {
                continue;
            };
            let destination = extracted.join(enclosed);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;
            fs::write(destination, contents)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SkillManifest;
    use std::io::Write;

    fn test_service() -> (AppService, tempfile::TempDir) {
        let service = AppService::in_memory().unwrap();
        let repository = tempfile::tempdir().unwrap();
        service
            .set_repository(&repository.path().to_string_lossy())
            .unwrap();
        (service, repository)
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

    #[test]
    fn imports_folder_skill() {
        let (service, _repository) = test_service();
        let upload = tempfile::tempdir().unwrap();
        write_demo_skill(upload.path(), "demo");

        let result = service.import_folder(upload.path()).unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 0);
        assert_eq!(service.scan_skills().unwrap().len(), 1);
    }

    #[test]
    fn skips_duplicate_skill() {
        let (service, _repository) = test_service();
        let upload = tempfile::tempdir().unwrap();
        write_demo_skill(upload.path(), "demo");

        service.import_folder(upload.path()).unwrap();
        let result = service.import_folder(upload.path()).unwrap();
        assert_eq!(result.imported, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn reports_empty_upload() {
        let (service, _repository) = test_service();
        let upload = tempfile::tempdir().unwrap();

        let result = service.import_folder(upload.path()).unwrap();
        assert_eq!(result.imported, 0);
        assert_eq!(result.skipped, 0);
    }

    #[test]
    fn imports_zip_skill() {
        let (service, _repository) = test_service();
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

        let result = service.import_zip_file(&zip_path).unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(service.scan_skills().unwrap().len(), 1);
    }
}
