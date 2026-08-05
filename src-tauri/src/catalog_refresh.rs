use crate::{
    catalog::{parse_clawhub_api_catalog_value, scan_catalog_repository},
    catalog_index::RefreshStatePatch,
    error::{AppError, AppResult},
    hash::copy_dir_all,
    models::{CatalogRefreshStatus, CatalogSafetyMode, CatalogSkill, CatalogSource, CatalogSourceKind},
    service::AppService,
    util::{safe_label, sanitize_zip_path},
};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::Duration,
};
use zip::ZipArchive;

/// ClawHub 单次下载的大小上限（zip 包），防止异常数据耗尽内存。
const MAX_CLAWHUB_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;

impl AppService {
    pub(crate) fn refresh_clawhub_index(&self, safety_mode: CatalogSafetyMode) -> AppResult<usize> {
        // begin_refresh 已由 start_catalog_refresh 调用，此处直接复用 generation，避免双重初始化
        let generation = self.catalog_index.refresh_status("clawhub", safety_mode)?.generation;
        let status = self.catalog_index.refresh_status("clawhub", safety_mode)?;
        let mut cursor = status.next_cursor.clone();
        let mut fetched_count = status.fetched_count;
        let source = built_in_catalog_sources()
            .into_iter()
            .find(|source| source.id == "clawhub")
            .ok_or_else(|| AppError::Message("找不到 ClawHub 内置源。".to_string()))?;
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(45))
            .build();

        loop {
            if self.is_catalog_refresh_cancelled("clawhub", safety_mode)? {
                self.save_catalog_refresh_cancelled(
                    "clawhub",
                    safety_mode,
                    cursor.as_deref(),
                    fetched_count,
                    generation,
                )?;
                return Ok(fetched_count);
            }

            let value = self.fetch_clawhub_index_page(&agent, cursor.as_deref(), safety_mode)?;
            if self.is_catalog_refresh_cancelled("clawhub", safety_mode)? {
                self.save_catalog_refresh_cancelled(
                    "clawhub",
                    safety_mode,
                    cursor.as_deref(),
                    fetched_count,
                    generation,
                )?;
                return Ok(fetched_count);
            }
            let skills = parse_clawhub_api_catalog_value(&value, &source)?;
            self.catalog_index
                .upsert_skills("clawhub", safety_mode, generation, &skills)?;
            fetched_count += skills.len();

            let next_cursor = value
                .get("nextCursor")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            self.catalog_index.save_refresh_state(RefreshStatePatch {
                source_id: "clawhub",
                safety_mode,
                cursor: next_cursor.as_deref(),
                fetched_count,
                generation,
                is_running: true,
                is_complete: false,
                last_error: None,
            })?;

            let Some(next_cursor) = next_cursor else {
                self.catalog_index.finish_refresh(
                    "clawhub",
                    safety_mode,
                    generation,
                    fetched_count,
                )?;
                return Ok(fetched_count);
            };
            cursor = Some(next_cursor);
        }
    }

    fn fetch_clawhub_index_page(
        &self,
        agent: &ureq::Agent,
        cursor: Option<&str>,
        safety_mode: CatalogSafetyMode,
    ) -> AppResult<serde_json::Value> {
        loop {
            let mut request = agent
                .get("https://clawhub.ai/api/v1/skills")
                .query("limit", "200")
                .query("sort", "createdAt")
                .query("dir", "desc");
            if let Some(cursor_value) = cursor {
                request = request.query("cursor", cursor_value);
            }
            if safety_mode == CatalogSafetyMode::NonSuspicious {
                request = request.query("nonSuspiciousOnly", "true");
            }
            match request.call() {
                Ok(response) => {
                    let text = response.into_string().map_err(|error| {
                        AppError::Message(format!("读取 ClawHub API 响应失败: {}", error))
                    })?;
                    return Ok(serde_json::from_str::<serde_json::Value>(&text)?);
                }
                Err(ureq::Error::Status(429, response)) => {
                    let wait = retry_after_delay(&response).min(Duration::from_secs(60));
                    if wait_for_retry_or_cancel(wait, Duration::from_millis(250), || {
                        self.is_catalog_refresh_cancelled("clawhub", safety_mode)
                    })? {
                        self.mark_catalog_refresh_cancelled("clawhub", safety_mode)?;
                        return Ok(serde_json::json!({ "items": [] }));
                    }
                }
                Err(error) => return Err(clawhub_http_error(error)),
            }
        }
    }

    pub(crate) fn mark_clawhub_refresh_error(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
        error: &str,
    ) -> AppResult<()> {
        let status = self.catalog_index.refresh_status(source_id, safety_mode)?;
        self.catalog_index.save_refresh_state(RefreshStatePatch {
            source_id,
            safety_mode,
            cursor: status.next_cursor.as_deref(),
            fetched_count: status.fetched_count,
            generation: status.generation,
            is_running: false,
            is_complete: false,
            last_error: Some(error),
        })
    }

    pub(crate) fn mark_catalog_refresh_cancelled(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
    ) -> AppResult<CatalogRefreshStatus> {
        let status = self.catalog_index.refresh_status(source_id, safety_mode)?;
        self.save_catalog_refresh_cancelled(
            source_id,
            safety_mode,
            status.next_cursor.as_deref(),
            status.fetched_count,
            status.generation,
        )?;
        self.catalog_index.refresh_status(source_id, safety_mode)
    }

    pub(crate) fn save_catalog_refresh_cancelled(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
        cursor: Option<&str>,
        fetched_count: usize,
        generation: i64,
    ) -> AppResult<()> {
        self.catalog_index.save_refresh_state(RefreshStatePatch {
            source_id,
            safety_mode,
            cursor,
            fetched_count,
            generation,
            is_running: false,
            is_complete: false,
            last_error: Some("用户已取消刷新"),
        })
    }

    pub(crate) fn is_catalog_refresh_cancelled(
        &self,
        source_id: &str,
        safety_mode: CatalogSafetyMode,
    ) -> AppResult<bool> {
        let cancel = self
            .catalog_refresh_cancel
            .lock()
            .map_err(|_| AppError::Message("Refresh cancel lock poisoned".to_string()))?;
        Ok(cancel.contains(&refresh_key(source_id, safety_mode)))
    }

    pub(crate) fn materialize_clawhub_skill(&self, skill: &CatalogSkill) -> AppResult<PathBuf> {
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

        let reader = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(60))
            .build()
            .get("https://clawhub.ai/api/v1/download")
            .query("slug", slug)
            .call()
            .map_err(clawhub_http_error)?
            .into_reader();
        let mut bytes = Vec::new();
        reader
            .take(MAX_CLAWHUB_DOWNLOAD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| AppError::Message(format!("下载 ClawHub skill 失败: {}", error)))?;
        if bytes.len() as u64 > MAX_CLAWHUB_DOWNLOAD_BYTES {
            return Err(AppError::Message(
                "ClawHub skill 下载超过大小上限。".to_string(),
            ));
        }
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

    pub(crate) fn unpack_zip_bytes(&self, bytes: &[u8], label: &str) -> AppResult<PathBuf> {
        let workspace = self.import_workspace(label)?;
        let extracted = workspace.join("expanded");
        fs::create_dir_all(&extracted)?;
        let mut archive = ZipArchive::new(Cursor::new(bytes))?;

        let mut extracted_count = 0u32;
        let mut skipped_count = 0u32;
        let mut total_extracted: u64 = 0;
        // zip bomb 防护：条目数 / 单文件 / 总解压大小上限
        const MAX_ZIP_ENTRIES: usize = 2000;
        const MAX_ZIP_FILE_SIZE: u64 = 50 * 1024 * 1024;
        const MAX_ZIP_TOTAL_SIZE: u64 = 500 * 1024 * 1024;
        if archive.len() > MAX_ZIP_ENTRIES {
            return Err(AppError::Message(format!(
                "zip 条目数量超过上限（{}），疑似 zip bomb。",
                MAX_ZIP_ENTRIES
            )));
        }

        for index in 0..archive.len() {
            let mut file = archive.by_index(index)?;
            if file.is_dir() {
                continue;
            }

            let file_size = file.size();
            if file_size > MAX_ZIP_FILE_SIZE {
                return Err(AppError::Message(format!(
                    "zip 中文件 {} 超过单文件大小上限。",
                    file.name()
                )));
            }
            total_extracted += file_size;
            if total_extracted > MAX_ZIP_TOTAL_SIZE {
                return Err(AppError::Message(
                    "zip 解压总大小超过上限，疑似 zip bomb。".to_string(),
                ));
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
            // 流式写盘，避免单文件整体读入内存
            let mut dest = fs::File::create(&destination)?;
            std::io::copy(&mut file, &mut dest)?;
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

pub(crate) fn clawhub_http_error(error: ureq::Error) -> AppError {
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

fn retry_after_delay(response: &ureq::Response) -> Duration {
    response
        .header("Retry-After")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(5))
}

fn wait_for_retry_or_cancel<F>(
    wait: Duration,
    interval: Duration,
    mut is_cancelled: F,
) -> AppResult<bool>
where
    F: FnMut() -> AppResult<bool>,
{
    let interval = interval.max(Duration::from_millis(1));
    let deadline = std::time::Instant::now() + wait;

    while std::time::Instant::now() < deadline {
        if is_cancelled()? {
            return Ok(true);
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        std::thread::sleep(remaining.min(interval));
    }

    is_cancelled()
}

pub(crate) fn refresh_key(source_id: &str, safety_mode: CatalogSafetyMode) -> String {
    format!("{}::{}", source_id, safety_mode.as_str())
}

pub(crate) fn built_in_catalog_sources() -> Vec<CatalogSource> {
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

#[cfg(test)]
pub(crate) fn built_in_catalog_sources_for_test() -> Vec<CatalogSource> {
    built_in_catalog_sources()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_wait_stops_when_cancelled() {
        let mut checks = 0usize;

        let cancelled =
            wait_for_retry_or_cancel(Duration::from_secs(60), Duration::from_millis(1), || {
                checks += 1;
                Ok(checks >= 2)
            })
            .unwrap();

        assert!(cancelled);
        assert_eq!(checks, 2);
    }

    #[test]
    fn collects_clawhub_pages_beyond_twenty_until_cursor_ends() {
        let mut pages = 0;
        let items = collect_clawhub_api_pages(|cursor| {
            pages += 1;
            if pages > 3 {
                return Ok(serde_json::json!({ "items": [] }));
            }
            let cursor_value = cursor.unwrap_or("start");
            Ok(serde_json::json!({
                "items": [{ "name": format!("skill-{}", cursor_value) }],
                "nextCursor": format!("cursor-{}", pages)
            }))
        })
        .unwrap();
        assert_eq!(pages, 4); // 3 data pages + 1 empty terminator
        assert_eq!(items.len(), 3);
    }

    fn collect_clawhub_api_pages<F>(mut fetch_page: F) -> AppResult<Vec<serde_json::Value>>
    where
        F: FnMut(Option<&str>) -> AppResult<serde_json::Value>,
    {
        const MAX_CLAWHUB_PAGES: usize = 1_000;
        const MAX_EMPTY_PAGES: usize = 3;

        let mut cursor: Option<String> = None;
        let mut seen_cursors = std::collections::HashSet::new();
        let mut empty_pages = 0usize;
        let mut items = Vec::new();

        for _ in 0..MAX_CLAWHUB_PAGES {
            let value = fetch_page(cursor.as_deref())?;
            let page_items = value
                .get("items")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();

            if page_items.is_empty() {
                empty_pages += 1;
                if empty_pages >= MAX_EMPTY_PAGES {
                    break;
                }
            } else {
                empty_pages = 0;
                items.extend(page_items);
            }

            let next_cursor = value
                .get("nextCursor")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);

            let Some(next_cursor) = next_cursor else {
                break;
            };
            if !seen_cursors.insert(next_cursor.clone()) {
                return Err(AppError::Message(
                    "ClawHub API 返回了重复的分页 cursor，已停止刷新以避免无限循环。".to_string(),
                ));
            }
            cursor = Some(next_cursor);
        }

        Ok(items)
    }
}
