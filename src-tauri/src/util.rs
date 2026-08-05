use crate::{
    error::{AppError, AppResult},
    models::{CatalogFilters, CatalogSearchResult, CatalogSkill},
};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    process::Command,
    time::SystemTime,
};

/// 校验相对路径不含目录穿越与绝对路径，防止文件操作逃逸出预期目录。
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

/// 将任意字符串转换为安全的目录名：保留字母数字（含 CJK 等 Unicode）与 _ -，其余替换为 -，并截断长度；
/// 避免中文名被全部映射成 ---- 导致不同技能的备份路径互相冲突。
pub(crate) fn safe_label(label: &str) -> String {
    let value = label
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .take(80)
        .collect::<String>();
    if value.is_empty() {
        "import".to_string()
    } else {
        value
    }
}

/// 规范化技能/目录标题为小写，用作分组 key。
pub(crate) fn normalize_title(title: &str) -> String {
    title.trim().to_lowercase()
}

/// Sanitize a raw zip entry path for safe extraction.
/// Returns None if the path is unsafe (absolute, contains traversal, or empty).
pub(crate) fn sanitize_zip_path(raw: &str) -> Option<PathBuf> {
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

/// 创建无控制台窗口的 Command（Windows 适用）。
pub(crate) fn command_no_window(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// 将 SystemTime 转换为 RFC3339 字符串。
pub(crate) fn system_time_to_rfc3339(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339()
}

// ---- Catalog 搜索辅助函数（从 service.rs 搬入）----

pub(crate) fn catalog_matches_query(skill: &CatalogSkill, query: &str) -> bool {
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

pub(crate) fn page_catalog_skills(
    skills: Vec<CatalogSkill>,
    page: Option<usize>,
    page_size: Option<usize>,
) -> CatalogSearchResult {
    let total = skills.len();
    let page = page.unwrap_or(1).max(1);
    let page_size = match page_size {
        Some(usize::MAX) => usize::MAX,
        Some(value) => value.clamp(1, 500),
        None => 100,
    };
    let start = page.saturating_sub(1).saturating_mul(page_size);
    let items = skills
        .into_iter()
        .skip(start)
        .take(page_size)
        .collect::<Vec<_>>();
    let has_more = start.saturating_add(items.len()) < total;

    CatalogSearchResult {
        items,
        total,
        page,
        page_size,
        has_more,
    }
}

pub(crate) fn catalog_matches_filters(skill: &CatalogSkill, filters: &CatalogFilters) -> bool {
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
        let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(updated_at) else {
            return false;
        };
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        if parsed.with_timezone(&chrono::Utc) < cutoff {
            return false;
        }
    }
    true
}

pub(crate) fn catalog_skill_is_installed(
    skill: &CatalogSkill,
    installed_titles: &HashSet<String>,
    installed_slugs: &HashSet<String>,
) -> bool {
    if skill.source_id == "clawhub" {
        return clawhub_skill_slug(skill)
            .map(|slug| installed_slugs.contains(&normalize_title(&slug)))
            .unwrap_or_else(|| installed_titles.contains(&normalize_title(&skill.name)));
    }
    installed_titles.contains(&normalize_title(&skill.name))
}

fn clawhub_skill_slug(skill: &CatalogSkill) -> Option<String> {
    skill
        .source_path
        .strip_prefix("clawhub://")
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or_else(|| (!skill.relative_path.trim().is_empty()).then(|| skill.relative_path.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_relative_path_rejects_unsafe_paths() {
        // 绝对路径（含 Windows 盘符与 POSIX 根）应被拒绝
        for bad in ["C:\\x", "C:/x", "/etc/passwd", "\\\\server\\share"] {
            assert!(safe_relative_path(bad).is_err(), "should reject: {bad}");
        }
        // 空路径 / 仅 . / .. 应被拒绝
        for bad in ["", ".", "..", "a/../b", "./../b"] {
            assert!(safe_relative_path(bad).is_err(), "should reject: {bad}");
        }
        // 合法相对路径应通过（含 CJK 目录名）
        for ok in ["a", "a/b/c.txt", "中文/技能/子目录"] {
            assert!(safe_relative_path(ok).is_ok(), "should accept: {ok}");
        }
    }

    #[test]
    fn sanitize_zip_path_blocks_traversal_and_absolute() {
        assert!(sanitize_zip_path("../escape").is_none());
        assert!(sanitize_zip_path("a/../../escape").is_none());
        assert!(sanitize_zip_path("/etc/passwd").is_none());
        assert!(sanitize_zip_path("C:\\x").is_none());
        assert!(sanitize_zip_path("a/b\0c").is_none());
        let result = sanitize_zip_path("dir/file.txt");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Path::new("dir/file.txt"));
    }

    #[test]
    fn safe_label_preserves_cjk_and_truncates() {
        assert_eq!(safe_label("学术论文写作流水线"), "学术论文写作流水线");
        assert_eq!(safe_label("a b!"), "a-b-");
        assert_eq!(safe_label(""), "import");
        // 超过 80 字符截断
        let long = "x".repeat(120);
        assert_eq!(safe_label(&long).len(), 80);
    }

    #[test]
    fn paginates_catalog_search_results_without_dropping_total_count() {
        let skills: Vec<CatalogSkill> = (0..15)
            .map(|i| CatalogSkill {
                id: format!("test::{i:03}"),
                name: format!("Skill {i:03}"),
                source_id: "test".to_string(),
                source_name: "Test".to_string(),
                source_icon: "test".to_string(),
                source_path: format!("test://skill-{i:03}"),
                relative_path: format!("skill-{i:03}"),
                description: None,
                tags: Vec::new(),
                supported_agents: Vec::new(),
                published_at: None,
                updated_at: None,
                download_count: None,
                install_count: None,
                has_skill_md: true,
                has_scripts: false,
                has_references: false,
                has_assets: false,
                install_status: crate::models::CatalogInstallStatus::NotInstalled,
            })
            .collect();
        // 第一页 10 条
        let page1 = page_catalog_skills(skills.clone(), Some(1), Some(10));
        assert_eq!(page1.items.len(), 10);
        assert_eq!(page1.total, 15);
        assert_eq!(page1.page, 1);
        assert!(page1.has_more);
        // 第二页 5 条
        let page2 = page_catalog_skills(skills, Some(2), Some(10));
        assert_eq!(page2.items.len(), 5);
        assert_eq!(page2.total, 15);
        assert_eq!(page2.page, 2);
        assert!(!page2.has_more);
        // 全量窗口（usize::MAX）用于安装前直查
        let all = page_catalog_skills(
            (0..3)
                .map(|i| CatalogSkill {
                    id: format!("test::{i:03}"),
                    name: format!("Skill {i:03}"),
                    source_id: "test".to_string(),
                    source_name: "Test".to_string(),
                    source_icon: "test".to_string(),
                    source_path: format!("test://skill-{i:03}"),
                    relative_path: format!("skill-{i:03}"),
                    description: None,
                    tags: Vec::new(),
                    supported_agents: Vec::new(),
                    published_at: None,
                    updated_at: None,
                    download_count: None,
                    install_count: None,
                    has_skill_md: true,
                    has_scripts: false,
                    has_references: false,
                    has_assets: false,
                    install_status: crate::models::CatalogInstallStatus::NotInstalled,
                })
                .collect(),
            Some(1),
            Some(usize::MAX),
        );
        assert_eq!(all.items.len(), 3);
    }
}
