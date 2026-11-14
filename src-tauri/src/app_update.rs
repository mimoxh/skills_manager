//! 程序自更新：从 GitHub Release 获取便携版 zip，替换当前 exe，由前端触发重启。
//! Windows 允许重命名正在运行的 exe，所以先把当前 exe 改名为 `.old.exe`，再把新 exe 放到原路径；
//! 下次启动时清理旧文件。

use crate::{
    error::{AppError, AppResult},
    models::{AppUpdateInfo, AppUpdateInstallResult},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::Duration,
};

pub const RELEASE_REPO: &str = "mimoxh/skills_manager";
const PORTABLE_ASSET_SUFFIX: &str = "-windows-portable.zip";
const PREFERRED_EXE_NAME: &str = "Skills Manager.exe";
const MAX_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;
const MAX_EXE_BYTES: u64 = 200 * 1024 * 1024;

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
    /// GitHub 为新上传资产提供 `sha256:<hex>`；旧资产可能缺失。
    #[serde(default)]
    pub digest: Option<String>,
}

/// 解析 `v1.2.3` / `1.2.3-beta.1`；预发布版本低于同号正式版。
pub fn parse_version(raw: &str) -> Option<(u64, u64, u64, bool)> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    let core_end = trimmed.find(['-', '+']).unwrap_or(trimmed.len());
    let core = &trimmed[..core_end];
    let pre = trimmed[core_end..].starts_with('-');
    let mut parts = core.split('.').map(|part| part.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    Some((major, minor, patch, pre))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some((a1, b1, c1, pre1)), Some((a2, b2, c2, pre2))) => {
            (a1, b1, c1, !pre1) > (a2, b2, c2, !pre2)
        }
        _ => false,
    }
}

pub fn select_portable_asset(release: &GithubRelease) -> Option<&GithubAsset> {
    release
        .assets
        .iter()
        .find(|asset| asset.name.ends_with(PORTABLE_ASSET_SUFFIX))
}

fn can_auto_install() -> bool {
    cfg!(windows) && !cfg!(debug_assertions)
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
}

pub fn fetch_latest_release() -> AppResult<GithubRelease> {
    let url = format!("https://api.github.com/repos/{RELEASE_REPO}/releases/latest");
    let response = http_agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", &format!("skills-manager/{}", current_version()))
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(404, _) => AppError::Message("GitHub 上还没有发布版本。".into()),
            ureq::Error::Status(403, _) => AppError::Message("GitHub API 访问受限（可能触发了频率限制），请稍后再试。".into()),
            other => AppError::Message(format!("查询最新版本失败：{other}")),
        })?;
    let text = response
        .into_string()
        .map_err(|error| AppError::Message(format!("读取版本信息失败：{error}")))?;
    Ok(serde_json::from_str(&text)?)
}

pub fn build_update_info(release: &GithubRelease, current: &str) -> AppUpdateInfo {
    let latest = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    let has_update = is_newer(&release.tag_name, current);
    let asset = select_portable_asset(release);
    let auto = can_auto_install();
    let message = if !has_update {
        Some("当前已是最新版本。".to_string())
    } else if asset.is_none() {
        Some("新版本还没有上传便携版 zip，请稍后再试或前往发布页手动下载。".to_string())
    } else if !auto {
        Some("当前运行形态不支持自动替换（开发构建或非 Windows），请前往发布页手动下载。".to_string())
    } else {
        None
    };
    AppUpdateInfo {
        current_version: current.to_string(),
        latest_version: Some(latest),
        has_update,
        release_name: release.name.clone().filter(|name| !name.trim().is_empty()),
        release_notes: release.body.clone().filter(|body| !body.trim().is_empty()),
        release_url: release.html_url.clone(),
        published_at: release.published_at.clone(),
        asset_name: asset.map(|asset| asset.name.clone()),
        asset_size: asset.map(|asset| asset.size),
        can_auto_install: auto && has_update && asset.is_some(),
        message,
    }
}

pub fn check_app_update() -> AppResult<AppUpdateInfo> {
    let release = fetch_latest_release()?;
    Ok(build_update_info(&release, current_version()))
}

fn download_asset(asset: &GithubAsset) -> AppResult<Vec<u8>> {
    let response = http_agent()
        .get(&asset.browser_download_url)
        .set("User-Agent", &format!("skills-manager/{}", current_version()))
        .call()
        .map_err(|error| AppError::Message(format!("下载更新包失败：{error}")))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_DOWNLOAD_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err(AppError::Message("更新包超过大小上限，已取消。".into()));
    }
    if asset.size > 0 && bytes.len() as u64 != asset.size {
        return Err(AppError::Message("更新包下载不完整，请重试。".into()));
    }
    verify_digest(&bytes, asset.digest.as_deref())?;
    Ok(bytes)
}

pub fn verify_digest(bytes: &[u8], digest: Option<&str>) -> AppResult<()> {
    let Some(expected) = digest.and_then(|value| value.strip_prefix("sha256:")) else {
        return Ok(());
    };
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(AppError::Message("更新包校验失败（SHA-256 不匹配），已取消。".into()))
    }
}

/// 从便携版 zip 中取出主程序；优先 `Skills Manager.exe`，否则取唯一的 exe。
pub fn extract_exe_from_zip(bytes: &[u8]) -> AppResult<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut candidates = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().replace('\\', "/");
        let base = name.rsplit('/').next().unwrap_or(&name).to_string();
        if base.to_ascii_lowercase().ends_with(".exe") {
            candidates.push((index, base));
        }
    }
    let index = candidates
        .iter()
        .find(|(_, base)| base == PREFERRED_EXE_NAME)
        .or_else(|| (candidates.len() == 1).then(|| &candidates[0]))
        .map(|(index, _)| *index)
        .ok_or_else(|| AppError::Message("更新包中找不到主程序 exe。".into()))?;
    let file = archive.by_index(index)?;
    let mut exe = Vec::new();
    file.take(MAX_EXE_BYTES + 1).read_to_end(&mut exe)?;
    if exe.len() as u64 > MAX_EXE_BYTES {
        return Err(AppError::Message("更新包中的 exe 超过大小上限，已取消。".into()));
    }
    if !exe.starts_with(b"MZ") {
        return Err(AppError::Message("更新包中的 exe 无效。".into()));
    }
    Ok(exe)
}

fn sibling_with_suffix(exe: &Path, suffix: &str) -> PathBuf {
    let stem = exe
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "SkillsManager".into());
    exe.with_file_name(format!("{stem}.{suffix}.exe"))
}

/// 把新 exe 放到 `exe` 路径；失败时恢复原文件。
pub fn swap_executable(exe: &Path, new_bytes: &[u8]) -> AppResult<()> {
    let staged = sibling_with_suffix(exe, "new");
    let old = sibling_with_suffix(exe, "old");
    fs::write(&staged, new_bytes).map_err(|error| {
        AppError::Message(format!(
            "无法写入程序目录（{error}）。请确认程序放在可写目录，或前往发布页手动下载。"
        ))
    })?;
    if old.exists() {
        if let Err(error) = fs::remove_file(&old) {
            let _ = fs::remove_file(&staged);
            return Err(AppError::Message(format!("无法清理上次更新留下的旧程序：{error}")));
        }
    }
    if let Err(error) = fs::rename(exe, &old) {
        let _ = fs::remove_file(&staged);
        return Err(AppError::Message(format!("无法替换当前程序：{error}")));
    }
    if let Err(error) = fs::rename(&staged, exe) {
        let _ = fs::rename(&old, exe);
        let _ = fs::remove_file(&staged);
        return Err(AppError::Message(format!("无法放置新程序，已恢复原程序：{error}")));
    }
    Ok(())
}

/// 启动时在后台清理上次更新留下的旧 exe。重启时旧进程可能还没退出，文件仍被占用，所以短暂重试。
pub fn cleanup_previous_update() {
    let Ok(exe) = std::env::current_exe() else { return };
    if !sibling_with_suffix(&exe, "old").exists() && !sibling_with_suffix(&exe, "new").exists() {
        return;
    }
    std::thread::spawn(move || {
        for _ in 0..20 {
            if cleanup_update_leftovers(&exe) {
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

/// 返回是否已无残留文件。
pub fn cleanup_update_leftovers(exe: &Path) -> bool {
    let mut clean = true;
    for suffix in ["old", "new"] {
        let path = sibling_with_suffix(exe, suffix);
        if path.exists() && fs::remove_file(&path).is_err() {
            clean = false;
        }
    }
    clean
}

pub fn install_app_update() -> AppResult<AppUpdateInstallResult> {
    if !can_auto_install() {
        return Err(AppError::Message(
            "当前运行形态不支持自动更新（开发构建或非 Windows），请前往发布页手动下载。".into(),
        ));
    }
    let release = fetch_latest_release()?;
    if !is_newer(&release.tag_name, current_version()) {
        return Err(AppError::Message("当前已是最新版本。".into()));
    }
    let asset = select_portable_asset(&release)
        .ok_or_else(|| AppError::Message("新版本还没有上传便携版 zip。".into()))?;
    let zip_bytes = download_asset(asset)?;
    let exe_bytes = extract_exe_from_zip(&zip_bytes)?;
    let exe = std::env::current_exe()
        .map_err(|error| AppError::Message(format!("无法获取当前程序路径：{error}")))?;
    swap_executable(&exe, &exe_bytes)?;
    let version = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    Ok(AppUpdateInstallResult {
        message: format!("已下载并替换为 v{version}，重启后生效。"),
        version,
        exe_path: exe.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn release(tag: &str, assets: Vec<GithubAsset>) -> GithubRelease {
        GithubRelease {
            tag_name: tag.into(),
            name: Some(format!("Skills Manager {tag}")),
            body: Some("notes".into()),
            html_url: Some("https://github.com/x/y/releases/tag/v".into()),
            published_at: None,
            assets,
        }
    }

    fn portable_asset() -> GithubAsset {
        GithubAsset {
            name: "SkillsManager-v0.6.0-windows-portable.zip".into(),
            browser_download_url: "https://example.invalid/a.zip".into(),
            size: 10,
            digest: None,
        }
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            for (name, data) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn version_comparison_handles_prefix_and_prerelease() {
        assert!(is_newer("v0.6.0", "0.5.0"));
        assert!(is_newer("0.5.1", "0.5.0"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("v0.5.0", "0.5.0"));
        assert!(!is_newer("0.4.9", "0.5.0"));
        assert!(!is_newer("0.6.0-beta.1", "0.6.0"));
        assert!(is_newer("0.6.0", "0.6.0-beta.1"));
        assert!(!is_newer("garbage", "0.5.0"));
    }

    #[test]
    fn update_info_reflects_release_and_asset() {
        let info = build_update_info(&release("v0.6.0", vec![portable_asset()]), "0.5.0");
        assert!(info.has_update);
        assert_eq!(info.latest_version.as_deref(), Some("0.6.0"));
        assert_eq!(info.asset_name.as_deref(), Some("SkillsManager-v0.6.0-windows-portable.zip"));

        let same = build_update_info(&release("v0.5.0", vec![portable_asset()]), "0.5.0");
        assert!(!same.has_update);
        assert!(!same.can_auto_install);

        let no_asset = build_update_info(&release("v0.6.0", vec![]), "0.5.0");
        assert!(no_asset.has_update);
        assert!(!no_asset.can_auto_install);
        assert!(no_asset.message.is_some());
    }

    #[test]
    fn digest_verification() {
        let data = b"hello";
        let good = format!("sha256:{:x}", Sha256::digest(data));
        assert!(verify_digest(data, Some(&good)).is_ok());
        assert!(verify_digest(data, Some("sha256:00")).is_err());
        assert!(verify_digest(data, None).is_ok());
    }

    #[test]
    fn extracts_preferred_exe_from_portable_zip() {
        let bytes = zip_with(&[
            ("Skills Manager/README.txt", b"readme"),
            ("Skills Manager/helper.exe", b"MZhelper"),
            ("Skills Manager/Skills Manager.exe", b"MZmain"),
        ]);
        assert_eq!(extract_exe_from_zip(&bytes).unwrap(), b"MZmain");

        let single = zip_with(&[("pkg/app.exe", b"MZonly")]);
        assert_eq!(extract_exe_from_zip(&single).unwrap(), b"MZonly");

        let invalid = zip_with(&[("pkg/Skills Manager.exe", b"not-pe")]);
        assert!(extract_exe_from_zip(&invalid).is_err());
        assert!(extract_exe_from_zip(&zip_with(&[("a.txt", b"x")])).is_err());
    }

    #[test]
    fn swap_replaces_exe_and_cleanup_removes_old() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("Skills Manager.exe");
        fs::write(&exe, b"MZold").unwrap();
        swap_executable(&exe, b"MZnew").unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"MZnew");
        let old = dir.path().join("Skills Manager.old.exe");
        assert_eq!(fs::read(&old).unwrap(), b"MZold");
        assert!(!dir.path().join("Skills Manager.new.exe").exists());

        // 再次更新时覆盖上次遗留的 old
        swap_executable(&exe, b"MZnewer").unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"MZnewer");
        assert_eq!(fs::read(&old).unwrap(), b"MZnew");

        assert!(cleanup_update_leftovers(&exe));
        assert!(!old.exists());
        assert!(exe.exists());
    }

    /// 联网验证真实发布：`cargo test app_update -- --ignored`
    #[test]
    #[ignore]
    fn real_release_asset_downloads_verifies_and_swaps() {
        let release = fetch_latest_release().unwrap();
        let asset = select_portable_asset(&release).expect("latest release has portable zip");
        let zip_bytes = download_asset(asset).unwrap();
        let exe = extract_exe_from_zip(&zip_bytes).unwrap();
        assert!(exe.len() > 1024 * 1024);
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Skills Manager.exe");
        fs::write(&target, b"MZold").unwrap();
        swap_executable(&target, &exe).unwrap();
        assert_eq!(fs::read(&target).unwrap().len(), exe.len());
    }

    #[test]
    fn swap_restores_original_when_target_missing() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("missing.exe");
        assert!(swap_executable(&exe, b"MZnew").is_err());
        assert!(!dir.path().join("missing.new.exe").exists());
    }
}
