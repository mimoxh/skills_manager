//! 同步运行时：把纯逻辑（`engine`）、打包/加密（`pack`/`crypto`）与传输（`transport`）
//! 组合成「发布本机清单」与「拉取远端并落中枢」两个动作。
//!
//! 该层只操作一个中枢目录（`hub_dir`）与本地 `AppStore` 的便携元数据，不触碰 agent
//! 派生目录；落中枢后的 fanout/reconcile 由上层（AppService）负责。
//!
//! bucket 布局：`blobs/<hash>.bin`、`devices/<deviceId>.json`、`_format.json`。
//! `_format.json` 明文存 `kdfSalt`（盐非秘密，需跨设备共享）。

use super::crypto::{Crypto, SALT_LEN};
use super::engine::{decide, MergeDecision, VersionState};
use super::pack::{pack_skill_dir, sha256_hex, unpack_zip_to_dir};
use super::transport::SyncTransport;
use crate::error::{AppError, AppResult};
use crate::hash::remove_dir_or_symlink;
use crate::models::{
    AgentProfile, AgentType, DeviceManifest, ManifestAgentMeta, ManifestSkillEntry, SyncConflict,
};
use crate::skill_scan::{load_skill_lock_map, read_agent_skill_info};
use crate::store::{AppStore, SyncSkillState};
use crate::util::normalize_title;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

const BLOBS_PREFIX: &str = "blobs/";
const DEVICES_PREFIX: &str = "devices/";
const FORMAT_KEY: &str = "_format.json";
const SCHEMA_VERSION: u32 = 1;

/// 本机中枢中的一个 skill（发布/合并的输入）。
#[derive(Debug, Clone)]
struct LocalSkill {
    key: String,
    name: String,
    dir_name: String,
    dir_path: PathBuf,
    version: Option<String>,
    source_url: Option<String>,
    installed_at: Option<String>,
    tags: Vec<String>,
    no_full_coverage: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PublishOutcome {
    pub skills: usize,
    pub blobs_uploaded: usize,
    pub tombstones: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct GcOutcome {
    pub removed: usize,
    pub kept: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PullOutcome {
    pub manifest_devices: usize,
    pub applied: usize,
    pub deleted: usize,
    pub conflicts: usize,
    /// 本次落中枢的 skill 目录名（用于上层 fanout / reconcile）。
    pub applied_dirs: Vec<String>,
    /// 本机首次获得的中枢 Skill；由上层提示用户选择本机目标。
    pub new_dirs: Vec<String>,
    /// 已从中枢删除的 (skill key, 目录名)，供上层安全移除管理器链接。
    pub deleted_dirs: Vec<(String, String)>,
}

pub struct SyncRuntime<'a> {
    store: &'a AppStore,
    transport: &'a dyn SyncTransport,
    crypto: Option<&'a Crypto>,
    hub_dir: PathBuf,
    device_id: String,
    device_name: String,
}

impl<'a> SyncRuntime<'a> {
    pub fn new(
        store: &'a AppStore,
        transport: &'a dyn SyncTransport,
        crypto: Option<&'a Crypto>,
        hub_dir: PathBuf,
    ) -> AppResult<Self> {
        fs::create_dir_all(&hub_dir)?;
        let device_id = store.ensure_device_id()?;
        let stored_name = store.device_name()?;
        let device_name = if stored_name.trim().is_empty() {
            device_id.clone()
        } else {
            stored_name
        };
        Ok(Self {
            store,
            transport,
            crypto,
            hub_dir,
            device_id,
            device_name,
        })
    }

    /// 发布本机中枢：上传缺失 blob，写入本设备清单（含墓碑与便携标签）。
    pub fn publish(&self) -> AppResult<PublishOutcome> {
        let existing_blobs: HashSet<String> =
            self.transport.list(BLOBS_PREFIX)?.into_iter().collect();
        // 读取本设备上一次发布的清单，用于识别"已删除"的 skill 并写墓碑。
        let previous_manifest: Option<DeviceManifest> = self
            .transport
            .get(&self.manifest_key())?
            .and_then(|bytes| self.unseal(&bytes, &self.device_id).ok())
            .and_then(|plain| serde_json::from_slice::<DeviceManifest>(&plain).ok());
        let previous_entries: HashMap<String, ManifestSkillEntry> = previous_manifest
            .as_ref()
            .map(|manifest| manifest.skills.clone())
            .unwrap_or_default();
        let conflict_keys: HashSet<String> = self
            .store
            .sync_conflicts()?
            .into_iter()
            .map(|c| c.skill_id)
            .collect();
        let baseline_map = self.store.list_sync_skill_state()?;
        let snapshots = self.local_snapshots()?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut entries: HashMap<String, ManifestSkillEntry> = HashMap::new();
        let mut outcome = PublishOutcome::default();

        for skill in &snapshots {
            let zip = pack_skill_dir(&skill.dir_path)?;
            let hash = sha256_hex(&zip);
            let blob_key = format!("{BLOBS_PREFIX}{hash}.bin");
            if !existing_blobs.contains(&blob_key) {
                let bytes = self.seal(&zip, &hash)?;
                self.transport.put(&blob_key, &bytes)?;
                outcome.blobs_uploaded += 1;
            }
            let entry_updated_at = previous_entries
                .get(&skill.key)
                .filter(|prev| {
                    !prev.deleted
                        && prev.hash == hash
                        && prev.tags == skill.tags
                        && prev.no_full_coverage == skill.no_full_coverage
                })
                .map(|prev| prev.updated_at.clone())
                .unwrap_or_else(|| now.clone());
            entries.insert(
                skill.key.clone(),
                ManifestSkillEntry {
                    name: skill.name.clone(),
                    dir_name: skill.dir_name.clone(),
                    hash: hash.clone(),
                    blob: blob_key,
                    version: skill.version.clone(),
                    source_url: skill.source_url.clone(),
                    installed_at: skill.installed_at.clone(),
                    tags: skill.tags.clone(),
                    category: None,
                    no_full_coverage: skill.no_full_coverage,
                    deleted: false,
                    updated_at: entry_updated_at,
                },
            );
            // 首次在本机发布的无冲突技能，初始化 baseline，避免对端修改后本机拉取误报冲突
            if !conflict_keys.contains(&skill.key)
                && baseline_map
                    .get(&skill.key)
                    .and_then(|s| s.last_merged_hash.as_ref())
                    .is_none()
            {
                self.update_baseline(&skill.key, Some(hash))?;
            }
        }
        outcome.skills = entries.len();

        // 墓碑：上次发布过、本次中枢已不存在的 skill；已存在的墓碑继续保留，避免跨端删除信号丢失。
        for (key, previous) in &previous_entries {
            if entries.contains_key(key) {
                continue;
            }
            if previous.deleted {
                entries.insert(key.clone(), previous.clone());
                continue;
            }
            entries.insert(
                key.clone(),
                ManifestSkillEntry {
                    name: previous.name.clone(),
                    dir_name: previous.dir_name.clone(),
                    hash: String::new(),
                    blob: String::new(),
                    version: None,
                    source_url: None,
                    installed_at: None,
                    tags: Vec::new(),
                    category: None,
                    no_full_coverage: false,
                    deleted: true,
                    updated_at: now.clone(),
                },
            );
            self.update_baseline(key, None)?;
            outcome.tombstones += 1;
        }

        // 补充墓碑：曾通过 pull 拉取落盘（有 baseline）但在首次 publish 前即被本机删除的 skill。
        for (key, state) in &baseline_map {
            if state.last_merged_hash.is_some()
                && !entries.contains_key(key)
                && !previous_entries.contains_key(key)
            {
                entries.insert(
                    key.clone(),
                    ManifestSkillEntry {
                        name: key.clone(),
                        dir_name: key.clone(),
                        hash: String::new(),
                        blob: String::new(),
                        version: None,
                        source_url: None,
                        installed_at: None,
                        tags: Vec::new(),
                        category: None,
                        no_full_coverage: false,
                        deleted: true,
                        updated_at: now.clone(),
                    },
                );
                self.update_baseline(key, None)?;
                outcome.tombstones += 1;
            }
        }

        let mut agents: HashMap<String, ManifestAgentMeta> = HashMap::new();
        for agent in self.store.list_agents()? {
            let key = portable_agent_key(&agent);
            let tags = self.store.list_agent_tags(&agent.id)?;
            if tags.is_empty() && !agents.contains_key(&key) {
                continue;
            }
            agents.insert(key, ManifestAgentMeta { user_tags: tags });
        }

        let manifest_updated_at = previous_manifest
            .as_ref()
            .filter(|prev| prev.agents == agents && outcome.tombstones == 0 && outcome.blobs_uploaded == 0)
            .and_then(|prev| {
                // 若所有条目与上次完全一致且 agent 标签未变，保留清单时间戳；否则刷新为 now
                let entries_unchanged = prev.skills.len() == entries.len()
                    && entries.iter().all(|(k, v)| {
                        prev.skills
                            .get(k)
                            .map(|p| p.updated_at == v.updated_at && p.deleted == v.deleted)
                            .unwrap_or(false)
                    });
                entries_unchanged.then(|| prev.updated_at.clone())
            })
            .unwrap_or_else(|| now.clone());

        let manifest = DeviceManifest {
            schema_version: SCHEMA_VERSION,
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            updated_at: manifest_updated_at,
            skills: entries,
            agents,
        };
        let json = serde_json::to_vec(&manifest)?;
        let bytes = self.seal_manifest(&json, &self.device_id)?;
        self.transport.put(&self.manifest_key(), &bytes)?;
        Ok(outcome)
    }

    /// 拉取并合并远端：下载 blob、安全解压落中枢、传播删除、汇总冲突。
    pub fn pull(&self) -> AppResult<PullOutcome> {
        let snapshots = self.local_snapshots()?;
        let local_by_key: HashMap<String, LocalSkill> = snapshots
            .iter()
            .map(|skill| (skill.key.clone(), skill.clone()))
            .collect();
        let mut local_hashes: HashMap<String, String> = HashMap::new();
        for skill in &snapshots {
            local_hashes.insert(skill.key.clone(), sha256_hex(&pack_skill_dir(&skill.dir_path)?));
        }

        // 读取其他设备清单
        let mut remotes: Vec<(String, DeviceManifest)> = Vec::new();
        for key in self.transport.list(DEVICES_PREFIX)? {
            let Some(file) = key.strip_prefix(DEVICES_PREFIX) else {
                continue;
            };
            let Some(device_id) = file.strip_suffix(".json") else {
                continue;
            };
            if device_id.is_empty() || device_id == self.device_id {
                continue;
            }
            let Some(bytes) = self.transport.get(&key)? else {
                continue;
            };
            let plain = self.unseal(&bytes, device_id)?;
            if let Ok(manifest) = serde_json::from_slice::<DeviceManifest>(&plain) {
                remotes.push((device_id.to_string(), manifest));
            }
        }

        let baseline_map = self.store.list_sync_skill_state()?;
        let mut keys: HashSet<String> = local_by_key.keys().cloned().collect();
        keys.extend(baseline_map.keys().cloned());
        for (_, manifest) in &remotes {
            keys.extend(manifest.skills.keys().cloned());
        }

        let now = chrono::Utc::now().to_rfc3339();
        let mut outcome = PullOutcome {
            manifest_devices: remotes.len(),
            ..Default::default()
        };
        let mut new_conflicts: Vec<SyncConflict> = Vec::new();

        for key in keys {
            let local_state = match local_by_key.get(&key) {
                Some(_) => Some(VersionState::present(
                    &self.device_id,
                    local_hashes
                        .get(&key)
                        .cloned()
                        .unwrap_or_default(),
                    now.clone(),
                )),
                None => {
                    let had_baseline = baseline_map
                        .get(&key)
                        .and_then(|state| state.last_merged_hash.as_ref())
                        .is_some();
                    if had_baseline {
                        Some(VersionState::tombstone(&self.device_id, now.clone()))
                    } else {
                        None
                    }
                }
            };

            let mut remote_states: Vec<VersionState> = Vec::new();
            let mut remote_entries: HashMap<String, &ManifestSkillEntry> = HashMap::new();
            for (device_id, manifest) in &remotes {
                if let Some(entry) = manifest.skills.get(&key) {
                    remote_states.push(if entry.deleted {
                        VersionState::tombstone(device_id, entry.updated_at.clone())
                    } else {
                        VersionState::present(device_id, entry.hash.clone(), entry.updated_at.clone())
                    });
                    remote_entries.insert(device_id.clone(), entry);
                }
            }

            let baseline = baseline_map
                .get(&key)
                .and_then(|state| state.last_merged_hash.clone());
            match decide(local_state.as_ref(), &remote_states, baseline.as_deref()) {
                MergeDecision::Noop => {
                    if let Some(local) = &local_state {
                        if !local.deleted {
                            let agreed = remote_states
                                .iter()
                                .any(|remote| !remote.deleted && remote.hash == local.hash);
                            if agreed {
                                self.update_baseline(&key, local.hash.clone())?;
                            }
                        }
                    }
                }
                MergeDecision::ApplyRemote { device_id, hash, .. } => {
                    if let Some(entry) = remote_entries.get(&device_id).copied() {
                        let is_new = !local_by_key.contains_key(&key);
                        self.apply_remote_entry(entry)?;
                        self.update_baseline(&key, Some(hash))?;
                        outcome.applied += 1;
                        outcome.applied_dirs.push(entry.dir_name.clone());
                        if is_new { outcome.new_dirs.push(entry.dir_name.clone()); }
                    }
                }
                MergeDecision::ApplyDelete { .. } => {
                    if let Some(local) = local_by_key.get(&key) {
                        remove_dir_or_symlink(&local.dir_path)?;
                        outcome.deleted_dirs.push((key.clone(), local.dir_name.clone()));
                    }
                    self.update_baseline(&key, None)?;
                    outcome.deleted += 1;
                }
                MergeDecision::Conflict(kind) => {
                    let local = local_state.as_ref();
                    let remote_hash = remote_states
                        .iter()
                        .find(|remote| !remote.deleted)
                        .and_then(|remote| remote.hash.clone());
                    let remote_device = remote_states
                        .iter()
                        .filter(|remote| !remote.deleted)
                        .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
                        .or_else(|| remote_states.iter().max_by(|a, b| a.updated_at.cmp(&b.updated_at)))
                        .map(|remote| remote.device_id.clone())
                        .unwrap_or_default();
                    let name = local_by_key
                        .get(&key)
                        .map(|skill| skill.name.clone())
                        .or_else(|| remote_entries.values().next().map(|entry| entry.name.clone()))
                        .unwrap_or_else(|| key.clone());
                    let dir_name = local_by_key
                        .get(&key)
                        .map(|skill| skill.dir_name.clone())
                        .or_else(|| remote_entries.values().next().map(|entry| entry.dir_name.clone()))
                        .unwrap_or_else(|| key.clone());
                    new_conflicts.push(SyncConflict {
                        skill_id: key.clone(),
                        name,
                        dir_name,
                        local_hash: local.and_then(|state| {
                            if state.deleted {
                                None
                            } else {
                                state.hash.clone()
                            }
                        }),
                        remote_hash,
                        remote_device_id: remote_device,
                        kind,
                        detected_at: now.clone(),
                    });
                    outcome.conflicts += 1;
                }
            }
        }

        self.merge_remote_tags(&local_by_key, &remotes)?;

        if !new_conflicts.is_empty() {
            let mut existing = self.store.sync_conflicts()?;
            for conflict in new_conflicts {
                if !existing
                    .iter()
                    .any(|current| current.skill_id == conflict.skill_id)
                {
                    existing.push(conflict);
                }
            }
            self.store.set_sync_conflicts(existing)?;
        }
        Ok(outcome)
    }

    fn apply_remote_entry(&self, entry: &ManifestSkillEntry) -> AppResult<()> {
        self.apply_remote_blob(&entry.dir_name, &entry.hash)
    }

    /// 按 hash 下载 blob、校验并落中枢指定目录（覆盖）。冲突解决复用。
    pub fn apply_remote_blob(&self, dir_name: &str, hash: &str) -> AppResult<()> {
        validate_skill_dir_name(dir_name)?;
        validate_sha256_hex(hash)?;
        let blob_key = format!("{BLOBS_PREFIX}{hash}.bin");
        let blob = self.transport.get(&blob_key)?.ok_or_else(|| {
            AppError::Message(format!("缺少远端 blob: {blob_key}"))
        })?;
        let plain = self.unseal(&blob, hash)?;
        let actual = sha256_hex(&plain);
        if actual != hash {
            return Err(AppError::Message(format!(
                "blob 校验失败: {dir_name}（期望 {hash}，实际 {actual}）"
            )));
        }
        let target = self.hub_dir.join(dir_name);
        let tmp = self.hub_dir.join(format!(
            ".sync-tmp-{}-{}",
            dir_name,
            chrono::Utc::now().timestamp_millis()
        ));
        let _ = remove_dir_or_symlink(&tmp);
        unpack_zip_to_dir(&plain, &tmp)?;
        remove_dir_or_symlink(&target)?;
        fs::rename(&tmp, &target)?;
        Ok(())
    }

    /// 清理未被任何设备清单引用的 blob（GC）。
    ///
    /// 仅手动调用：若另一设备刚上传 blob 但尚未写清单，可能被误删；因此不做自动 GC。
    /// 若任何设备清单读取、解密或解析失败，立即中止并返回错误，绝不误删对象。
    pub fn gc(&self) -> AppResult<GcOutcome> {
        let blobs = self.transport.list(BLOBS_PREFIX)?;
        if blobs.is_empty() {
            return Ok(GcOutcome::default());
        }
        let mut referenced: HashSet<String> = HashSet::new();
        for key in self.transport.list(DEVICES_PREFIX)? {
            let Some(file) = key.strip_prefix(DEVICES_PREFIX) else {
                continue;
            };
            let Some(device_id) = file.strip_suffix(".json") else {
                continue;
            };
            if device_id.is_empty() {
                continue;
            }
            let bytes = self.transport.get(&key)?.ok_or_else(|| {
                AppError::Message(format!("GC 中止：无法读取设备清单 {key}"))
            })?;
            let plain = self.unseal(&bytes, device_id).map_err(|err| {
                AppError::Message(format!("GC 中止：设备清单 {key} 解密失败: {err}"))
            })?;
            let manifest = serde_json::from_slice::<DeviceManifest>(&plain).map_err(|err| {
                AppError::Message(format!("GC 中止：设备清单 {key} 解析失败: {err}"))
            })?;
            for entry in manifest.skills.values() {
                if !entry.deleted && !entry.blob.is_empty() {
                    referenced.insert(entry.blob.clone());
                }
            }
        }
        let mut outcome = GcOutcome::default();
        for blob in blobs {
            if referenced.contains(&blob) {
                outcome.kept += 1;
            } else {
                self.transport.delete(&blob)?;
                outcome.removed += 1;
            }
        }
        Ok(outcome)
    }

    /// 更新某 skill 的合并基准（冲突解决后调用）。
    pub fn update_baseline(&self, key: &str, hash: Option<String>) -> AppResult<()> {
        let existing = self.store.sync_skill_state(key)?.unwrap_or_default();
        if existing.last_merged_hash == hash {
            return Ok(());
        }
        self.store.set_sync_skill_state(
            key,
            SyncSkillState {
                last_merged_hash: hash,
                last_synced_at: Some(chrono::Utc::now().to_rfc3339()),
            },
        )?;
        Ok(())
    }

    fn seal(&self, bytes: &[u8], nonce_id: &str) -> AppResult<Vec<u8>> {
        match self.crypto {
            Some(crypto) => crypto.encrypt(bytes, nonce_id),
            None => Ok(bytes.to_vec()),
        }
    }

    fn seal_manifest(&self, bytes: &[u8], aad_id: &str) -> AppResult<Vec<u8>> {
        match self.crypto {
            Some(crypto) => crypto.encrypt_random_nonce(bytes, aad_id),
            None => Ok(bytes.to_vec()),
        }
    }

    fn unseal(&self, bytes: &[u8], nonce_id: &str) -> AppResult<Vec<u8>> {
        match self.crypto {
            Some(crypto) => crypto.decrypt(bytes, nonce_id),
            None => Ok(bytes.to_vec()),
        }
    }

    fn manifest_key(&self) -> String {
        format!("{DEVICES_PREFIX}{}.json", self.device_id)
    }

    fn local_snapshots(&self) -> AppResult<Vec<LocalSkill>> {
        let no_coverage: HashSet<String> = self
            .store
            .list_no_full_coverage()?
            .into_iter()
            .map(|title| normalize_title(&title))
            .collect();
        let lock_map = load_skill_lock_map();
        let mut skills = Vec::new();
        for entry in fs::read_dir(&self.hub_dir)? {
            let entry = entry?;
            let dir_path = entry.path();
            if !dir_path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if dir_name.starts_with('.') {
                continue;
            }
            let (title, version, _description, _readme) =
                read_agent_skill_info(&dir_path, false);
            let name = if title.trim().is_empty() {
                dir_name.clone()
            } else {
                title
            };
            let key = normalize_title(&name);
            if key.is_empty() {
                continue;
            }
            let no_full_coverage = no_coverage.contains(&key);
            let lock = lock_map.get(&dir_name);
            skills.push(LocalSkill {
                key,
                name,
                dir_name,
                dir_path,
                version,
                source_url: lock.and_then(|entry| entry.source_url.clone()),
                installed_at: lock.and_then(|entry| entry.installed_at.clone()),
                tags: Vec::new(),
                no_full_coverage,
            });
        }
        // tags 需在 name 上取，用 store 查询填充
        for skill in &mut skills {
            skill.tags = self.store.list_skill_tags(&skill.name)?;
        }
        skills.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(skills)
    }

    fn merge_remote_tags(
        &self,
        local: &HashMap<String, LocalSkill>,
        remotes: &[(String, DeviceManifest)],
    ) -> AppResult<()> {
        if remotes.is_empty() {
            return Ok(());
        }
        // 读取本机已发布的清单（用于比较最新时间戳 LWW）
        let self_manifest: Option<DeviceManifest> = self
            .transport
            .get(&self.manifest_key())?
            .and_then(|bytes| self.unseal(&bytes, &self.device_id).ok())
            .and_then(|plain| serde_json::from_slice::<DeviceManifest>(&plain).ok());

        let normalize_tags = |tags: &[String]| -> Vec<String> {
            let mut merged: Vec<String> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            for tag in tags {
                let trimmed = tag.trim();
                if !trimmed.is_empty() && seen.insert(trimmed.to_lowercase()) {
                    merged.push(trimmed.to_string());
                }
            }
            merged.sort_by_key(|value| value.to_lowercase());
            merged
        };

        // 1. skill 标签与 no_full_coverage：按最新时间戳（LWW）覆盖
        let mut latest_remote_per_key: HashMap<String, &ManifestSkillEntry> = HashMap::new();
        for (_, manifest) in remotes {
            for (key, entry) in &manifest.skills {
                if entry.deleted {
                    continue;
                }
                match latest_remote_per_key.get(key) {
                    Some(existing) if existing.updated_at >= entry.updated_at => {}
                    _ => {
                        latest_remote_per_key.insert(key.clone(), entry);
                    }
                }
            }
        }

        let current_no_cov: HashSet<String> = self
            .store
            .list_no_full_coverage()?
            .into_iter()
            .map(|t| normalize_title(&t))
            .collect();

        for (key, remote_entry) in latest_remote_per_key {
            let name = local
                .get(&key)
                .map(|skill| skill.name.clone())
                .unwrap_or_else(|| remote_entry.name.clone());
            let self_entry = self_manifest
                .as_ref()
                .and_then(|m| m.skills.get(&key))
                .filter(|e| !e.deleted);

            let should_apply_remote = match self_entry {
                Some(se) => remote_entry.updated_at >= se.updated_at,
                None => true,
            };
            if should_apply_remote {
                let target_tags = normalize_tags(&remote_entry.tags);
                let local_tags = normalize_tags(&self.store.list_skill_tags(&name)?);
                if local_tags != target_tags {
                    self.store.set_skill_tags(&name, target_tags)?;
                }
                let is_marked = current_no_cov.contains(&key);
                if is_marked != remote_entry.no_full_coverage {
                    let _ = self.store.toggle_no_full_coverage(&name)?;
                }
            }
        }

        // 2. agent 标签：按便携键与清单 updated_at 最新时间戳（LWW）覆盖
        let self_updated_at = self_manifest.as_ref().map(|m| m.updated_at.as_str());
        for agent in self.store.list_agents()? {
            let key = portable_agent_key(&agent);
            let mut latest_remote: Option<(&str, &ManifestAgentMeta)> = None;
            for (_, manifest) in remotes {
                if let Some(meta) = manifest.agents.get(&key) {
                    match latest_remote {
                        Some((ts, _)) if ts >= manifest.updated_at.as_str() => {}
                        _ => {
                            latest_remote = Some((manifest.updated_at.as_str(), meta));
                        }
                    }
                }
            }
            if let Some((remote_ts, meta)) = latest_remote {
                let should_apply = match self_updated_at {
                    Some(local_ts) => remote_ts >= local_ts,
                    None => true,
                };
                if should_apply {
                    let target_tags = normalize_tags(&meta.user_tags);
                    let local_tags = normalize_tags(&self.store.list_agent_tags(&agent.id)?);
                    if local_tags != target_tags {
                        self.store.set_agent_tags(&agent.id, target_tags)?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn validate_skill_dir_name(dir_name: &str) -> AppResult<()> {
    let trimmed = dir_name.trim();
    if trimmed.is_empty() || trimmed.starts_with('.') {
        return Err(AppError::Message(format!("非法远端技能目录名: {dir_name}")));
    }
    let safe = crate::util::safe_relative_path(trimmed)?;
    if safe.components().count() != 1 {
        return Err(AppError::Message(format!(
            "远端技能目录名必须是单层目录: {dir_name}"
        )));
    }
    Ok(())
}

fn validate_sha256_hex(hash: &str) -> AppResult<()> {
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::Message(format!("非法远端 blob 哈希: {hash}")));
    }
    Ok(())
}

/// agent 便携键：内置类型用 `AgentType::as_str()`，custom 用名称。
fn portable_agent_key(agent: &AgentProfile) -> String {
    if agent.agent_type == AgentType::Custom {
        agent.name.trim().to_string()
    } else {
        agent.agent_type.as_str().to_string()
    }
}

/// 读取或创建共享 KDF 盐（明文存 bucket `_format.json`）。
///
/// 并发创建的处理：写入后重新读取一次，双方最终收敛到持久化的同一盐，
/// 避免两台设备首次同时启用时各生成一个盐导致无法互相解密。
pub fn ensure_kdf_salt(transport: &dyn SyncTransport) -> AppResult<[u8; SALT_LEN]> {
    if let Some(salt) = read_kdf_salt(transport)? {
        return Ok(salt);
    }
    let salt = Crypto::generate_salt();
    let value = serde_json::json!({ "schemaVersion": SCHEMA_VERSION, "kdfSalt": hex_encode(&salt) });
    transport.put(FORMAT_KEY, &serde_json::to_vec(&value)?)?;
    // 重新读取：若与其它设备并发写入，取最终持久化值。
    if let Some(existing) = read_kdf_salt(transport)? {
        return Ok(existing);
    }
    Ok(salt)
}

fn read_kdf_salt(transport: &dyn SyncTransport) -> AppResult<Option<[u8; SALT_LEN]>> {
    let Some(bytes) = transport.get(FORMAT_KEY)? else {
        return Ok(None);
    };
    let value = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|err| {
        AppError::Message(format!("远端 _format.json 损坏，拒绝覆盖加密盐: {err}"))
    })?;
    let Some(salt) = value
        .get("kdfSalt")
        .and_then(|value| value.as_str())
        .and_then(hex_decode)
    else {
        return Err(AppError::Message(
            "远端 _format.json 缺少有效的 kdfSalt 字段，拒绝覆盖加密盐".to_string(),
        ));
    };
    if salt.len() != SALT_LEN {
        return Err(AppError::Message(
            "远端 _format.json 中 kdfSalt 长度无效，拒绝覆盖加密盐".to_string(),
        ));
    }
    let mut out = [0u8; SALT_LEN];
    out.copy_from_slice(&salt);
    Ok(Some(out))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let hi = (bytes[index] as char).to_digit(16)?;
        let lo = (bytes[index + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        index += 2;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentProfile, AgentType};
    use crate::sync::LocalDirTransport;
    use std::io::Write;
    use std::path::Path;

    fn write_skill(root: &Path, dir: &str, content: &str) {
        let path = root.join(dir);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), content).unwrap();
    }

    fn device(_bucket: &LocalDirTransport, state_dir: &Path, _hub: &Path) -> AppStore {
        let store = AppStore::with_data_dir(state_dir.to_path_buf()).unwrap();
        // 预置设备名便于可读
        store.set_device_name("tester").unwrap();
        store.ensure_device_id().unwrap();
        store
    }

    fn crypto(salt: &[u8]) -> Crypto {
        Crypto::from_password("test-password", salt).unwrap()
    }

    #[test]
    fn round_trip_publishes_and_pulls_a_skill() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let salt = [9u8; SALT_LEN];
        let crypto = crypto(&salt);

        let state_a = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let store_a = device(&transport, state_a.path(), hub_a.path());
        write_skill(hub_a.path(), "demo", "# Demo");
        let runtime_a = SyncRuntime::new(&store_a, &transport, Some(&crypto), hub_a.path().to_path_buf()).unwrap();
        let published = runtime_a.publish().unwrap();
        assert_eq!(published.skills, 1);
        assert_eq!(published.blobs_uploaded, 1);

        let state_b = tempfile::tempdir().unwrap();
        let hub_b = tempfile::tempdir().unwrap();
        let store_b = device(&transport, state_b.path(), hub_b.path());
        let runtime_b = SyncRuntime::new(&store_b, &transport, Some(&crypto), hub_b.path().to_path_buf()).unwrap();
        let pulled = runtime_b.pull().unwrap();
        assert_eq!(pulled.manifest_devices, 1);
        assert_eq!(pulled.applied, 1);
        assert!(hub_b.path().join("demo").join("SKILL.md").exists());
        assert_eq!(
            fs::read_to_string(hub_b.path().join("demo").join("SKILL.md")).unwrap(),
            "# Demo"
        );
    }

    #[test]
    fn deletion_propagates_via_tombstone() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let salt = [3u8; SALT_LEN];
        let crypto = crypto(&salt);

        let state_a = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let store_a = device(&transport, state_a.path(), hub_a.path());
        write_skill(hub_a.path(), "demo", "# Demo");
        let runtime_a = SyncRuntime::new(&store_a, &transport, Some(&crypto), hub_a.path().to_path_buf()).unwrap();
        runtime_a.publish().unwrap();

        let state_b = tempfile::tempdir().unwrap();
        let hub_b = tempfile::tempdir().unwrap();
        let store_b = device(&transport, state_b.path(), hub_b.path());
        let runtime_b = SyncRuntime::new(&store_b, &transport, Some(&crypto), hub_b.path().to_path_buf()).unwrap();
        runtime_b.pull().unwrap();
        assert!(hub_b.path().join("demo").exists());

        // A 删除并发布墓碑
        remove_dir_or_symlink(&hub_a.path().join("demo")).unwrap();
        let published = runtime_a.publish().unwrap();
        assert_eq!(published.tombstones, 1);

        let pulled = runtime_b.pull().unwrap();
        assert_eq!(pulled.deleted, 1);
        assert!(!hub_b.path().join("demo").exists());
    }

    #[test]
    fn conflict_is_detected_on_divergent_edits() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let salt = [5u8; SALT_LEN];
        let crypto = crypto(&salt);

        let state_a = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let store_a = device(&transport, state_a.path(), hub_a.path());
        write_skill(hub_a.path(), "demo", "v1");
        let runtime_a = SyncRuntime::new(&store_a, &transport, Some(&crypto), hub_a.path().to_path_buf()).unwrap();
        runtime_a.publish().unwrap();

        let state_b = tempfile::tempdir().unwrap();
        let hub_b = tempfile::tempdir().unwrap();
        let store_b = device(&transport, state_b.path(), hub_b.path());
        let runtime_b = SyncRuntime::new(&store_b, &transport, Some(&crypto), hub_b.path().to_path_buf()).unwrap();
        runtime_b.pull().unwrap();

        // 两端各自修改
        fs::write(hub_a.path().join("demo").join("SKILL.md"), "v2-from-a").unwrap();
        runtime_a.publish().unwrap();
        fs::write(hub_b.path().join("demo").join("SKILL.md"), "v2-from-b").unwrap();
        runtime_b.publish().unwrap();

        let pulled_b = runtime_b.pull().unwrap();
        assert_eq!(pulled_b.conflicts, 1);
        let conflicts = store_b.sync_conflicts().unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].skill_id, "demo");
    }

    #[test]
    fn tags_merge_across_devices() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let salt = [11u8; SALT_LEN];
        let crypto = crypto(&salt);

        let state_a = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let store_a = device(&transport, state_a.path(), hub_a.path());
        write_skill(hub_a.path(), "demo", "# Demo");
        store_a.set_skill_tags("demo", vec!["AI".into()]).unwrap();
        let runtime_a = SyncRuntime::new(&store_a, &transport, Some(&crypto), hub_a.path().to_path_buf()).unwrap();
        runtime_a.publish().unwrap();

        let state_b = tempfile::tempdir().unwrap();
        let hub_b = tempfile::tempdir().unwrap();
        let store_b = device(&transport, state_b.path(), hub_b.path());
        store_b.set_skill_tags("demo", vec!["写作".into()]).unwrap();
        let runtime_b = SyncRuntime::new(&store_b, &transport, Some(&crypto), hub_b.path().to_path_buf()).unwrap();
        runtime_b.pull().unwrap();

        // 最新时间戳覆盖（LWW）：A 发布了 ["AI"]，B 拉取后以最新远端为准覆盖为 ["AI"]
        let tags = store_b.list_skill_tags("demo").unwrap();
        assert_eq!(tags, vec!["AI".to_string()]);

        // B 随后修改标签为 ["写作"]（并清空 "AI"）且标记 no_full_coverage 后发布，A 拉取应被覆盖为 ["写作"]
        store_b.set_skill_tags("demo", vec!["写作".into()]).unwrap();
        store_b.toggle_no_full_coverage("demo").unwrap();
        runtime_b.publish().unwrap();
        runtime_a.pull().unwrap();
        assert_eq!(store_a.list_skill_tags("demo").unwrap(), vec!["写作".to_string()]);
        assert!(store_a.list_no_full_coverage().unwrap().contains(&"Demo".to_string()));
    }

    fn custom_agent(id: &str, name: &str) -> AgentProfile {
        AgentProfile {
            id: id.into(),
            name: name.into(),
            agent_type: AgentType::Custom,
            skills_path: "/tmp".into(),
            adapter_config: None,
            user_tags: Vec::new(),
            supports_universal: false,
        }
    }

    #[test]
    fn agent_tags_merge_across_devices() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let salt = [13u8; SALT_LEN];
        let crypto = crypto(&salt);

        let state_a = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let store_a = device(&transport, state_a.path(), hub_a.path());
        store_a.save_agent(&custom_agent("a1", "My Agent")).unwrap();
        store_a.set_agent_tags("a1", vec!["AI".into()]).unwrap();
        let runtime_a = SyncRuntime::new(&store_a, &transport, Some(&crypto), hub_a.path().to_path_buf()).unwrap();
        runtime_a.publish().unwrap();

        let state_b = tempfile::tempdir().unwrap();
        let hub_b = tempfile::tempdir().unwrap();
        let store_b = device(&transport, state_b.path(), hub_b.path());
        store_b.save_agent(&custom_agent("b1", "My Agent")).unwrap();
        store_b.set_agent_tags("b1", vec!["写作".into()]).unwrap();
        let runtime_b = SyncRuntime::new(&store_b, &transport, Some(&crypto), hub_b.path().to_path_buf()).unwrap();
        runtime_b.pull().unwrap();

        // 最新时间戳覆盖（LWW）：B 拉取后覆盖为 A 最新发布的 ["AI"]
        let tags = store_b.list_agent_tags("b1").unwrap();
        assert_eq!(tags, vec!["AI".to_string()]);

        // B 更新为 ["写作"] 并发布后，A 拉取应更新为 ["写作"]
        store_b.set_agent_tags("b1", vec!["写作".into()]).unwrap();
        runtime_b.publish().unwrap();
        runtime_a.pull().unwrap();
        assert_eq!(store_a.list_agent_tags("a1").unwrap(), vec!["写作".to_string()]);
    }

    #[test]
    fn gc_removes_unreferenced_blobs() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let salt = [17u8; SALT_LEN];
        let crypto = crypto(&salt);

        let state_a = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let store_a = device(&transport, state_a.path(), hub_a.path());
        write_skill(hub_a.path(), "demo", "# Demo");
        let runtime_a =
            SyncRuntime::new(&store_a, &transport, Some(&crypto), hub_a.path().to_path_buf())
                .unwrap();
        runtime_a.publish().unwrap();

        // 插入一个未被引用的孤儿 blob
        transport.put("blobs/orphan.bin", b"orphan").unwrap();
        let outcome = runtime_a.gc().unwrap();
        assert_eq!(outcome.removed, 1);
        assert_eq!(outcome.kept, 1);
        assert!(transport.get("blobs/orphan.bin").unwrap().is_none());
        // 被引用的 blob 仍在
        assert_eq!(transport.list("blobs/").unwrap().len(), 1);
    }

    #[test]
    fn kdf_salt_is_created_once_and_shared() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let first = ensure_kdf_salt(&transport).unwrap();
        let second = ensure_kdf_salt(&transport).unwrap();
        assert_eq!(first, second);
        // 另一台设备读到同一盐
        let other = ensure_kdf_salt(&transport).unwrap();
        assert_eq!(first, other);
    }

    #[test]
    fn portable_agent_key_uses_name_for_custom() {
        let custom = AgentProfile {
            id: "a".into(),
            name: "My Agent".into(),
            agent_type: AgentType::Custom,
            skills_path: "/tmp".into(),
            adapter_config: None,
            user_tags: Vec::new(),
            supports_universal: false,
        };
        assert_eq!(portable_agent_key(&custom), "My Agent");
        let claude = AgentProfile {
            agent_type: AgentType::ClaudeCode,
            ..custom
        };
        assert_eq!(portable_agent_key(&claude), "claudeCode");
    }

    #[test]
    fn zip_unpack_rejects_traversal() {
        // 构造一个包含 ../escape 的 zip
        let mut buffer = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("../escape.txt", options).unwrap();
            writer.write_all(b"bad").unwrap();
            writer.finish().unwrap();
        }
        let target = tempfile::tempdir().unwrap();
        let out = target.path().join("out");
        // enclosed_name 会拒绝 ../，条目被跳过，不会逃逸到 target 根之外
        unpack_zip_to_dir(&buffer, &out).unwrap();
        assert!(!target.path().join("escape.txt").exists());
    }

    #[test]
    fn tombstone_persists_across_multiple_publishes() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let salt = [21u8; SALT_LEN];
        let crypto = crypto(&salt);

        let state_a = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let store_a = device(&transport, state_a.path(), hub_a.path());
        write_skill(hub_a.path(), "demo", "# Demo");
        let runtime_a = SyncRuntime::new(&store_a, &transport, Some(&crypto), hub_a.path().to_path_buf()).unwrap();
        runtime_a.publish().unwrap();

        let state_b = tempfile::tempdir().unwrap();
        let hub_b = tempfile::tempdir().unwrap();
        let store_b = device(&transport, state_b.path(), hub_b.path());
        let runtime_b = SyncRuntime::new(&store_b, &transport, Some(&crypto), hub_b.path().to_path_buf()).unwrap();
        runtime_b.pull().unwrap();

        // A 删除技能并连续发布两次（模拟 60s 轮询再次触发 publish）
        remove_dir_or_symlink(&hub_a.path().join("demo")).unwrap();
        runtime_a.publish().unwrap();
        runtime_a.publish().unwrap();

        // B 此时上线 pull，仍应收到删除墓碑
        let pulled = runtime_b.pull().unwrap();
        assert_eq!(pulled.deleted, 1);
        assert!(!hub_b.path().join("demo").exists());
    }

    #[test]
    fn gc_aborts_when_any_device_manifest_fails_to_unseal() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let salt = [23u8; SALT_LEN];
        let crypto = crypto(&salt);

        let state_a = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let store_a = device(&transport, state_a.path(), hub_a.path());
        write_skill(hub_a.path(), "demo", "# Demo");
        let runtime_a = SyncRuntime::new(&store_a, &transport, Some(&crypto), hub_a.path().to_path_buf()).unwrap();
        runtime_a.publish().unwrap();

        // 写入一个损坏的设备清单
        transport.put("devices/device-corrupt.json", b"corrupted-ciphertext").unwrap();
        assert!(runtime_a.gc().is_err());
        // 原有 blob 绝不能被误删
        assert_eq!(transport.list("blobs/").unwrap().len(), 1);
    }

    #[test]
    fn apply_remote_blob_rejects_path_traversal_and_invalid_hash() {
        let bucket_dir = tempfile::tempdir().unwrap();
        let transport = LocalDirTransport::new(bucket_dir.path()).unwrap();
        let state_a = tempfile::tempdir().unwrap();
        let hub_a = tempfile::tempdir().unwrap();
        let store_a = device(&transport, state_a.path(), hub_a.path());
        let runtime = SyncRuntime::new(&store_a, &transport, None, hub_a.path().to_path_buf()).unwrap();

        let valid_hash = "a".repeat(64);
        assert!(runtime.apply_remote_blob("../escape", &valid_hash).is_err());
        assert!(runtime.apply_remote_blob(".", &valid_hash).is_err());
        assert!(runtime.apply_remote_blob("", &valid_hash).is_err());
        assert!(runtime.apply_remote_blob("a/b", &valid_hash).is_err());
        assert!(runtime.apply_remote_blob("demo", "not-a-valid-sha256").is_err());
    }
}
