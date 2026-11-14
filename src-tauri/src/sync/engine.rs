//! 同步引擎的**纯逻辑**部分：合并、墓碑与冲突判定。
//!
//! 这里不接触网络与文件系统，输入是各设备的"版本状态"，输出是应采取的动作，
//! 便于用单元测试固化冲突/快进/删除传播/三方合并等规则（见 `docs/skills-sync-plan.md` §4.8）。
//!
//! 基准 `baseline = last_merged_hash`：上次各端达成一致的内容哈希。
//! 判定思路：把所有设备（本机 + 远端）中相对基准**发生变化**的状态收集起来，
//! - 无变化 → `Noop`；
//! - 只有一种新内容 → 快进（若本机已是该内容则 `Noop`）；
//! - 多种新内容 → 冲突（删除与修改并存则 `DeleteVsModify`，否则 `BothModified`）。

use crate::models::{ManifestAgentMeta, ManifestSkillEntry, SyncConflictKind};
use std::collections::HashSet;

/// 单个设备对某 skill 的当前状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionState {
    pub device_id: String,
    /// `Some(hash)` 表示存在；`None` 表示不存在（与 `deleted` 配合表示墓碑）。
    pub hash: Option<String>,
    pub deleted: bool,
    pub updated_at: String,
}

impl VersionState {
    pub fn present(device_id: impl Into<String>, hash: impl Into<String>, updated_at: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            hash: Some(hash.into()),
            deleted: false,
            updated_at: updated_at.into(),
        }
    }

    pub fn tombstone(device_id: impl Into<String>, updated_at: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            hash: None,
            deleted: true,
            updated_at: updated_at.into(),
        }
    }

    fn content(&self) -> Content {
        if self.deleted || self.hash.is_none() {
            Content::Deleted
        } else {
            Content::Hash(self.hash.clone().unwrap())
        }
    }

    fn changed(&self, baseline: Option<&str>) -> bool {
        match self.content() {
            Content::Deleted => baseline.is_some(),
            Content::Hash(hash) => Some(hash.as_str()) != baseline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Content {
    Hash(String),
    Deleted,
}

/// 引擎对某 skill 的决定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeDecision {
    /// 无需动作（本机已是最新，或本机就是变更来源）。
    Noop,
    /// 快进：采用远端内容。
    ApplyRemote {
        device_id: String,
        hash: String,
        updated_at: String,
    },
    /// 删除传播：远端删除了该 skill，需要在本机落墓碑。
    ApplyDelete { device_id: String, updated_at: String },
    /// 真冲突：需要用户选择。
    Conflict(SyncConflictKind),
}

/// 综合本机与远端状态，决定下一步动作。
pub fn decide(
    local: Option<&VersionState>,
    remotes: &[VersionState],
    baseline: Option<&str>,
) -> MergeDecision {
    let mut states: Vec<&VersionState> = Vec::new();
    if let Some(local) = local {
        states.push(local);
    }
    states.extend(remotes.iter());

    let changed: Vec<&VersionState> = states
        .iter()
        .copied()
        .filter(|state| state.changed(baseline))
        .collect();
    if changed.is_empty() {
        return MergeDecision::Noop;
    }

    let mut distinct: Vec<Content> = Vec::new();
    for state in &changed {
        let content = state.content();
        if !distinct.contains(&content) {
            distinct.push(content);
        }
    }

    if distinct.len() > 1 {
        let has_deleted = distinct.iter().any(|content| *content == Content::Deleted);
        let has_hash = distinct.iter().any(|content| matches!(content, Content::Hash(_)));
        let kind = if has_deleted && has_hash {
            SyncConflictKind::DeleteVsModify
        } else {
            SyncConflictKind::BothModified
        };
        return MergeDecision::Conflict(kind);
    }

    let target = distinct.into_iter().next().expect("非空");
    let local_device = local.map(|state| state.device_id.as_str());

    // 本机已是要应用的内容 → 无需动作。
    if let Some(local) = local {
        if local.changed(baseline) && local.content() == target {
            return MergeDecision::Noop;
        }
    }

    // 从远端里选一个承载目标内容、updatedAt 最新的状态。
    let candidate = changed
        .iter()
        .copied()
        .filter(|state| Some(state.device_id.as_str()) != local_device)
        .filter(|state| state.content() == target)
        .max_by(|a, b| a.updated_at.cmp(&b.updated_at));

    let Some(candidate) = candidate else {
        return MergeDecision::Noop;
    };

    match target {
        Content::Deleted => MergeDecision::ApplyDelete {
            device_id: candidate.device_id.clone(),
            updated_at: candidate.updated_at.clone(),
        },
        Content::Hash(hash) => MergeDecision::ApplyRemote {
            device_id: candidate.device_id.clone(),
            hash,
            updated_at: candidate.updated_at.clone(),
        },
    }
}

/// 标签合并：并集，按小写去重，结果按小写排序保证确定性。
pub fn merge_tags<'a>(entries: impl IntoIterator<Item = &'a ManifestSkillEntry>) -> Vec<String> {
    merge_string_sets(entries.into_iter().map(|entry| entry.tags.iter()))
}

/// agent 标签合并：并集，规则同 `merge_tags`。
pub fn merge_agent_tags<'a>(metas: impl IntoIterator<Item = &'a ManifestAgentMeta>) -> Vec<String> {
    merge_string_sets(metas.into_iter().map(|meta| meta.user_tags.iter()))
}

fn merge_string_sets<'a>(
    sets: impl IntoIterator<Item = std::slice::Iter<'a, String>>,
) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut merged: Vec<String> = Vec::new();
    for set in sets {
        for value in set {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                continue;
            }
            if seen.insert(trimmed.to_lowercase()) {
                merged.push(trimmed.to_string());
            }
        }
    }
    merged.sort_by_key(|value| value.to_lowercase());
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_remotes_is_noop() {
        let local = VersionState::present("local", "h1", "2026-09-11T00:00:00Z");
        assert_eq!(decide(Some(&local), &[], Some("h1")), MergeDecision::Noop);
    }

    #[test]
    fn fast_forwards_when_only_remote_changed() {
        let local = VersionState::present("local", "h1", "2026-09-11T00:00:00Z");
        let remote = VersionState::present("mac", "h2", "2026-09-11T01:00:00Z");
        assert_eq!(
            decide(Some(&local), &[remote], Some("h1")),
            MergeDecision::ApplyRemote {
                device_id: "mac".into(),
                hash: "h2".into(),
                updated_at: "2026-09-11T01:00:00Z".into(),
            }
        );
    }

    #[test]
    fn adopts_remote_on_first_pull_without_local() {
        let remote = VersionState::present("mac", "h2", "2026-09-11T01:00:00Z");
        assert_eq!(
            decide(None, &[remote], None),
            MergeDecision::ApplyRemote {
                device_id: "mac".into(),
                hash: "h2".into(),
                updated_at: "2026-09-11T01:00:00Z".into(),
            }
        );
    }

    #[test]
    fn local_only_change_is_noop() {
        // 本机改了尚未发布，远端还在基准 → 上传即可，不拉取
        let local = VersionState::present("local", "h2", "2026-09-11T02:00:00Z");
        let remote = VersionState::present("mac", "h1", "2026-09-11T00:00:00Z");
        assert_eq!(decide(Some(&local), &[remote], Some("h1")), MergeDecision::Noop);
    }

    #[test]
    fn both_modified_same_content_is_noop() {
        let local = VersionState::present("local", "h2", "2026-09-11T02:00:00Z");
        let remote = VersionState::present("mac", "h2", "2026-09-11T01:00:00Z");
        assert_eq!(decide(Some(&local), &[remote], Some("h1")), MergeDecision::Noop);
    }

    #[test]
    fn true_conflict_when_both_changed_differently() {
        let local = VersionState::present("local", "h2", "2026-09-11T02:00:00Z");
        let remote = VersionState::present("mac", "h3", "2026-09-11T01:00:00Z");
        assert_eq!(
            decide(Some(&local), &[remote], Some("h1")),
            MergeDecision::Conflict(SyncConflictKind::BothModified)
        );
    }

    #[test]
    fn bootstrap_collision_is_conflict() {
        let local = VersionState::present("local", "hl", "2026-09-11T02:00:00Z");
        let remote = VersionState::present("mac", "hr", "2026-09-11T01:00:00Z");
        assert_eq!(
            decide(Some(&local), &[remote], None),
            MergeDecision::Conflict(SyncConflictKind::BothModified)
        );
    }

    #[test]
    fn propagates_deletion_when_remote_deletes() {
        let local = VersionState::present("local", "h1", "2026-09-11T00:00:00Z");
        let remote = VersionState::tombstone("mac", "2026-09-11T03:00:00Z");
        assert_eq!(
            decide(Some(&local), &[remote], Some("h1")),
            MergeDecision::ApplyDelete {
                device_id: "mac".into(),
                updated_at: "2026-09-11T03:00:00Z".into(),
            }
        );
    }

    #[test]
    fn delete_vs_modify_is_conflict() {
        let local = VersionState::present("local", "h2", "2026-09-11T02:00:00Z");
        let remote = VersionState::tombstone("mac", "2026-09-11T03:00:00Z");
        assert_eq!(
            decide(Some(&local), &[remote], Some("h1")),
            MergeDecision::Conflict(SyncConflictKind::DeleteVsModify)
        );
    }

    #[test]
    fn local_tombstone_is_noop_when_remote_unchanged() {
        let local = VersionState::tombstone("local", "2026-09-11T03:00:00Z");
        let remote = VersionState::present("mac", "h1", "2026-09-11T00:00:00Z");
        assert_eq!(decide(Some(&local), &[remote], Some("h1")), MergeDecision::Noop);
    }

    #[test]
    fn three_way_merge_picks_newest_single_change() {
        let local = VersionState::present("local", "h1", "2026-09-11T00:00:00Z");
        let mac = VersionState::present("mac", "h1", "2026-09-11T00:00:00Z");
        let linux = VersionState::present("linux", "h2", "2026-09-11T05:00:00Z");
        assert_eq!(
            decide(Some(&local), &[mac, linux], Some("h1")),
            MergeDecision::ApplyRemote {
                device_id: "linux".into(),
                hash: "h2".into(),
                updated_at: "2026-09-11T05:00:00Z".into(),
            }
        );
    }

    #[test]
    fn two_remotes_differing_is_conflict() {
        let mac = VersionState::present("mac", "h2", "2026-09-11T01:00:00Z");
        let linux = VersionState::present("linux", "h3", "2026-09-11T05:00:00Z");
        assert_eq!(
            decide(None, &[mac, linux], Some("h1")),
            MergeDecision::Conflict(SyncConflictKind::BothModified)
        );
    }

    #[test]
    fn merge_tags_unions_and_dedups_case_insensitively() {
        let a = ManifestSkillEntry {
            name: "A".into(),
            dir_name: "a".into(),
            hash: "h".into(),
            blob: "blobs/h".into(),
            version: None,
            source_url: None,
            installed_at: None,
            tags: vec!["AI".into(), "写作".into()],
            category: None,
            no_full_coverage: false,
            deleted: false,
            updated_at: "t".into(),
        };
        let mut b = a.clone();
        b.tags = vec!["ai".into(), "工具".into()];
        let merged = merge_tags([&a, &b]);
        assert_eq!(merged, vec!["AI".to_string(), "写作".into(), "工具".into()]);
    }

    #[test]
    fn merge_agent_tags_unions() {
        let a = ManifestAgentMeta {
            user_tags: vec!["生产力".into()],
        };
        let b = ManifestAgentMeta {
            user_tags: vec!["ai".into()],
        };
        assert_eq!(
            merge_agent_tags([&a, &b]),
            vec!["ai".to_string(), "生产力".into()]
        );
    }
}
